use crate::asset_service;
use crate::business_closure_service;
use crate::contract_review_service;
use crate::document_engine;
use crate::protocol::{
    AdoptLatestConfirmedRequirementPayload, AssetDomainEvent, AssignBusinessCustomerPayload,
    AttachBusinessInvoiceAssetPayload, BusinessArchiveIntegrityStatus, BusinessCurrentDocuments,
    BusinessCustomerReceivableSummary, BusinessDocumentFormat, BusinessDocumentKind,
    BusinessDocumentRecord, BusinessDocumentSnapshot, BusinessDocumentStatus,
    BusinessEvidenceInput, BusinessEvidenceKind, BusinessEvidenceRecord, BusinessFinancialSummary,
    BusinessInvoiceKind, BusinessLifecycleStage, BusinessLineItem, BusinessLineItemInput,
    BusinessManualWaiverInput, BusinessManualWaiverRecord, BusinessPaymentInput,
    BusinessPaymentRecord, BusinessPaymentStatus, BusinessProfile, BusinessProfileInput,
    BusinessQuoteConfirmationRecord, BusinessReceiptKind, BusinessReceiptRecord,
    BusinessWorkspaceCommandEnvelope, BusinessWorkspaceCommandResponse,
    BusinessWorkspaceDomainEvent, BusinessWorkspaceEventType, BusinessWorkspacePrefillCandidate,
    BusinessWorkspacePrefillChange, BusinessWorkspacePrefillDecision,
    BusinessWorkspacePrefillField, BusinessWorkspacePrefillMatchKind,
    BusinessWorkspacePrefillPreview, BusinessWorkspaceRecord, BusinessWorkspaceStatus,
    ChangeBusinessDocumentStatusPayload, ChangeBusinessWorkspaceStatusPayload, CommandReceipt,
    ConfirmBusinessQuotePayload, CreateBusinessArchiveSnapshotPayload,
    CreateBusinessDocumentPayload, CreateBusinessWorkspacePayload, GenerateBusinessDocumentPayload,
    HostError, ListBusinessCustomersRequest, ListBusinessWorkspacePrefillCandidatesRequest,
    OperationContext, PreviewBusinessWorkspacePrefillRequest, PromoteReviewedContractPayload,
    RecordBusinessDeliverySentPayload, RecordBusinessDeliverySignoffPayload,
    RecordBusinessInvoiceIssuedPayload, RecordBusinessInvoiceRedCorrectionPayload,
    RecordBusinessReceiptPayload, RegisterBusinessDeliverableVersionPayload,
    RequirementBriefContent, ReverseBusinessReceiptPayload, UpdateBusinessProfilePayload,
    UpsertBusinessCustomerPayload, UpsertBusinessMilestonePayload, UpsertBusinessPaymentPayload,
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
const MAX_DOCUMENTS_PER_WORKSPACE: i64 = 1_000;
const MAX_RECEIPTS_PER_WORKSPACE: i64 = 5_000;
const DEFAULT_CUSTOMER_LIST_LIMIT: u32 = 100;
const MAX_CUSTOMER_LIST_LIMIT: u32 = 500;
const MAX_MONEY_CENTS: i64 = 9_000_000_000_000_000;
const MAX_QUANTITY_MILLIS: i64 = 1_000_000_000_000;
const MAX_REPLAY_LIMIT: u32 = 1_000;
const DEFAULT_PREFILL_CANDIDATE_LIMIT: u32 = 50;
const MAX_PREFILL_CANDIDATE_LIMIT: u32 = 100;
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
                FOREIGN KEY(output_asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(report_asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_documents_workspace
                ON business_documents(workspace_id, created_at ASC, id ASC);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_business_documents_output_asset
                ON business_documents(output_asset_id) WHERE output_asset_id IS NOT NULL;

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
                     'businessWorkspace.paymentUpserted','businessWorkspace.quoteConfirmed',
                     'businessWorkspace.receiptRecorded','businessWorkspace.receiptReversed',
                     'businessWorkspace.requirementAdopted','businessWorkspace.customerUpserted',
                     'businessWorkspace.customerAssigned','businessWorkspace.milestoneUpserted',
                     'businessWorkspace.deliverableVersionRegistered','businessWorkspace.deliverySent',
                     'businessWorkspace.deliverySignoffRecorded','businessWorkspace.invoiceIssued',
                     'businessWorkspace.invoiceRedCorrected','businessWorkspace.invoiceAssetAttached',
                     'businessWorkspace.archiveSnapshotPrepared',
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
                     'businessWorkspace.upsertPayment','businessWorkspace.confirmQuote',
                     'businessWorkspace.recordReceipt','businessWorkspace.reverseReceipt',
                     'businessWorkspace.adoptLatestConfirmedRequirement','businessWorkspace.upsertCustomer',
                     'businessWorkspace.assignCustomer','businessWorkspace.upsertMilestone',
                     'businessWorkspace.registerDeliverableVersion','businessWorkspace.recordDeliverySent',
                     'businessWorkspace.recordDeliverySignoff','businessWorkspace.recordInvoiceIssued',
                     'businessWorkspace.recordInvoiceRedCorrection','businessWorkspace.attachInvoiceAsset',
                     'businessWorkspace.createArchiveSnapshot',
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
    migrate_reviewed_contract_binding(connection)?;
    ensure_workspace_lifecycle_columns(connection)?;
    ensure_document_lifecycle_columns(connection)?;
    migrate_payment_ledger_schema(connection)?;
    ensure_quote_confirmation_schema(connection)?;
    ensure_receipt_schema(connection)?;
    migrate_legacy_received_payments(connection)?;
    business_closure_service::migrate(connection)?;
    ensure_event_audit_columns(connection)?;
    migrate_event_type_constraint(connection)?;
    migrate_receipt_protocol_constraint(connection)
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
    Ok(())
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
        && table_sql.contains("businessWorkspace.quoteConfirmed")
        && table_sql.contains("businessWorkspace.receiptRecorded")
        && table_sql.contains("businessWorkspace.receiptReversed")
        && table_sql.contains("businessWorkspace.requirementAdopted")
        && table_sql.contains("businessWorkspace.archiveSnapshotPrepared")
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
                     'businessWorkspace.paymentUpserted','businessWorkspace.quoteConfirmed',
                     'businessWorkspace.receiptRecorded','businessWorkspace.receiptReversed',
                     'businessWorkspace.requirementAdopted','businessWorkspace.customerUpserted',
                     'businessWorkspace.customerAssigned','businessWorkspace.milestoneUpserted',
                     'businessWorkspace.deliverableVersionRegistered','businessWorkspace.deliverySent',
                     'businessWorkspace.deliverySignoffRecorded','businessWorkspace.invoiceIssued',
                     'businessWorkspace.invoiceRedCorrected','businessWorkspace.invoiceAssetAttached',
                     'businessWorkspace.archiveSnapshotPrepared','businessWorkspace.statusChanged')),
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
        && table_sql.contains("businessWorkspace.confirmQuote")
        && table_sql.contains("businessWorkspace.recordReceipt")
        && table_sql.contains("businessWorkspace.reverseReceipt")
        && table_sql.contains("businessWorkspace.adoptLatestConfirmedRequirement")
        && table_sql.contains("businessWorkspace.createArchiveSnapshot")
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
                     'businessWorkspace.upsertPayment','businessWorkspace.confirmQuote',
                     'businessWorkspace.recordReceipt','businessWorkspace.reverseReceipt',
                     'businessWorkspace.adoptLatestConfirmedRequirement','businessWorkspace.upsertCustomer',
                     'businessWorkspace.assignCustomer','businessWorkspace.upsertMilestone',
                     'businessWorkspace.registerDeliverableVersion','businessWorkspace.recordDeliverySent',
                     'businessWorkspace.recordDeliverySignoff','businessWorkspace.recordInvoiceIssued',
                     'businessWorkspace.recordInvoiceRedCorrection','businessWorkspace.attachInvoiceAsset',
                     'businessWorkspace.createArchiveSnapshot',
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
    if matches!(command, NormalizedCommand::GenerateDocument { .. }) {
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
                 WHERE (origin.origin = 'businessDocument' AND d.id IS NULL)
                    OR (origin.origin IN ('generatedArchiveManifest','generatedArchivePackage')
                        AND snapshot.id IS NULL)",
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
        if origin == "businessDocument"
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
            | Self::PromoteReviewedContract { meta, .. }
            | Self::ChangeDocumentStatus { meta, .. }
            | Self::GenerateDocument { meta, .. }
            | Self::UpsertPayment { meta, .. }
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
            | Self::ChangeStatus { meta, .. } => meta,
        }
    }

    fn command_type(&self) -> &'static str {
        match self {
            Self::Create { .. } => "businessWorkspace.create",
            Self::UpdateProfile { .. } => "businessWorkspace.updateProfile",
            Self::CreateDocument { .. } => "businessWorkspace.createDocument",
            Self::PromoteReviewedContract { .. } => "businessWorkspace.promoteReviewedContract",
            Self::ChangeDocumentStatus { .. } => "businessWorkspace.changeDocumentStatus",
            Self::GenerateDocument { .. } => "businessWorkspace.generateDocument",
            Self::UpsertPayment { .. } => "businessWorkspace.upsertPayment",
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
                },
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

fn execute_generate_document(
    connection: &mut Connection,
    vault_root: &Path,
    command: NormalizedCommand,
    fingerprint: String,
) -> Result<BusinessWorkspaceCommandOutcome, HostError> {
    let NormalizedCommand::GenerateDocument { meta, payload } = command else {
        unreachable!("generate dispatcher received another command")
    };

    let document = {
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
        ensure_document_generatable(&document, &payload.format)?;
        ensure_document_prerequisites(&workspace, &document.kind)?;
        if document.kind == BusinessDocumentKind::Contract {
            ensure_current_quote_confirmed(&transaction, vault_root, &workspace)?;
        }
        ensure_positive_document_total(&document)?;
        ensure_payment_request_target(&workspace, &document)?;
        transaction.commit().map_err(sql_error)?;
        document
    };

    let generation_id = Uuid::new_v4().to_string();
    let staged = document_engine::generate_document(vault_root, &document, &payload.format)?;
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
                serde_json::to_string(&response).map_err(json_error)?,
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
        payments: Vec::new(),
        quote_confirmations: Vec::new(),
        receipts: Vec::new(),
        milestones: Vec::new(),
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
        let normalized = normalize_line_item(id, item)?;
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
    let tax_multiplier = 10_000_i128 + i128::from(input.tax_rate_bps);
    let denominator = 1_000_i128 * 10_000_i128;
    let taxed_numerator = subtotal_numerator
        .checked_mul(tax_multiplier)
        .ok_or_else(|| {
            HostError::validation("computed line item amount exceeds the supported range")
        })?;
    let rounded = (taxed_numerator + denominator / 2) / denominator;
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
    let document = BusinessDocumentRecord {
        id: Uuid::new_v4().to_string(),
        kind: payload.kind.clone(),
        sequence_number,
        document_number: payload.document_number.clone(),
        title: payload.title.clone(),
        template_key: payload.template_key.clone(),
        status: BusinessDocumentStatus::Draft,
        snapshot: BusinessDocumentSnapshot {
            workspace_revision: workspace.revision,
            customer_id: workspace.customer_id.clone(),
            customer: workspace.customer.clone(),
            profile: workspace.profile.clone(),
            payment,
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
        created_at: now,
        updated_at: now,
    };
    transaction
        .execute(
            "INSERT INTO business_documents
             (id, workspace_id, kind, sequence_number, document_number, title,
              template_key, status, snapshot_json, output_asset_id, output_format,
              approved_at, approved_by, generated_at, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'draft', ?8,
                     NULL, NULL, NULL, NULL, NULL, 1, ?9, ?9)",
            params![
                document.id,
                workspace.id,
                document_kind_to_db(&document.kind),
                document.sequence_number,
                document.document_number,
                document.title,
                document.template_key,
                serde_json::to_string(&document.snapshot).map_err(json_error)?,
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
    ensure_document_prerequisites(&workspace, &BusinessDocumentKind::Contract)?;
    ensure_current_quote_confirmed(transaction, vault_root, &workspace)?;
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
    document_engine::validate_template(&document.kind, &document.template_key)?;
    let valid_format = matches!(
        (&document.kind, format),
        (BusinessDocumentKind::Quote, BusinessDocumentFormat::Xlsx)
            | (
                BusinessDocumentKind::Contract
                    | BusinessDocumentKind::PaymentRequest
                    | BusinessDocumentKind::Acceptance,
                BusinessDocumentFormat::Docx
            )
    );
    if !valid_format {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_FORMAT_INVALID",
            "quote documents require XLSX; all other business documents require DOCX",
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
    ensure_document_generatable(document, &payload.format)?;
    ensure_document_prerequisites(&workspace, &document.kind)?;
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
) -> BusinessProfile {
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
    proposed
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
    let proposed = merge_confirmed_requirement(&workspace.profile, latest.2);
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
                    prefill_source_workspace_id, profile_json, status, archived_at, archived_by,
                    revision, created_at, updated_at
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
        payments,
        quote_confirmations,
        receipts,
        milestones,
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
    document
        .snapshot
        .profile
        .line_items
        .iter()
        .fold(0_i64, |total, item| total.saturating_add(item.amount_cents))
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
    let status_value: String = row.get(6)?;
    Ok(WorkspaceBase {
        id: row.get(0)?,
        project_id: row.get(1)?,
        requirement_brief_id: row.get(2)?,
        requirement_brief_revision: row.get(3)?,
        prefill_source_workspace_id: row.get(4)?,
        profile,
        status: workspace_status_from_db(&status_value)?,
        archived_at: row.get(7)?,
        archived_by: row.get(8)?,
        revision: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
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
                    approved_at, approved_by, generated_at, revision, created_at, updated_at
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

fn document_from_row(row: &Row<'_>) -> rusqlite::Result<BusinessDocumentRecord> {
    let kind: String = row.get(1)?;
    let status: String = row.get(6)?;
    let snapshot_json: String = row.get(7)?;
    let format: Option<String> = row.get(9)?;
    Ok(BusinessDocumentRecord {
        id: row.get(0)?,
        kind: document_kind_from_db(&kind)?,
        sequence_number: row.get(2)?,
        document_number: row.get(3)?,
        title: row.get(4)?,
        template_key: row.get(5)?,
        status: document_status_from_db(&status)?,
        snapshot: from_json_column(&snapshot_json)?,
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
        "businessWorkspace.registerDeliverableVersion" => "登记交付物版本",
        "businessWorkspace.recordDeliverySent" => "登记交付发送",
        "businessWorkspace.recordDeliverySignoff" => "登记客户签收",
        "businessWorkspace.recordInvoiceIssued" => "登记开票",
        "businessWorkspace.recordInvoiceRedCorrection" => "登记发票红冲",
        "businessWorkspace.attachInvoiceAsset" => "补充发票附件",
        "businessWorkspace.createArchiveSnapshot" => "生成归档完整性快照",
        "businessWorkspace.changeStatus" => "变更商务工作区状态",
        _ => "执行商务工作台命令",
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
                serde_json::to_string(workspace).map_err(json_error)?,
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
        business_workspace: workspace.clone(),
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
               AND origin.origin IN ('businessDocument','generatedArchiveManifest','generatedArchivePackage')",
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
                     AND origin IN ('businessDocument','generatedArchiveManifest','generatedArchivePackage')
               )
               AND NOT EXISTS(
                   SELECT 1 FROM business_documents WHERE output_asset_id = ?1
               )
               AND NOT EXISTS(
                   SELECT 1 FROM business_archive_snapshots
                   WHERE manifest_asset_id = ?1 OR package_asset_id = ?1
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
        BusinessWorkspaceEventType::QuoteConfirmed => "businessWorkspace.quoteConfirmed",
        BusinessWorkspaceEventType::ReceiptRecorded => "businessWorkspace.receiptRecorded",
        BusinessWorkspaceEventType::ReceiptReversed => "businessWorkspace.receiptReversed",
        BusinessWorkspaceEventType::RequirementAdopted => "businessWorkspace.requirementAdopted",
        BusinessWorkspaceEventType::CustomerUpserted => "businessWorkspace.customerUpserted",
        BusinessWorkspaceEventType::CustomerAssigned => "businessWorkspace.customerAssigned",
        BusinessWorkspaceEventType::MilestoneUpserted => "businessWorkspace.milestoneUpserted",
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
        "businessWorkspace.quoteConfirmed" => Ok(BusinessWorkspaceEventType::QuoteConfirmed),
        "businessWorkspace.receiptRecorded" => Ok(BusinessWorkspaceEventType::ReceiptRecorded),
        "businessWorkspace.receiptReversed" => Ok(BusinessWorkspaceEventType::ReceiptReversed),
        "businessWorkspace.requirementAdopted" => {
            Ok(BusinessWorkspaceEventType::RequirementAdopted)
        }
        "businessWorkspace.customerUpserted" => Ok(BusinessWorkspaceEventType::CustomerUpserted),
        "businessWorkspace.customerAssigned" => Ok(BusinessWorkspaceEventType::CustomerAssigned),
        "businessWorkspace.milestoneUpserted" => Ok(BusinessWorkspaceEventType::MilestoneUpserted),
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
    use crate::protocol::{AssetKind, BriefRecord};
    use std::fs::File;
    use std::io::Read;
    use zip::ZipArchive;

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
        source_workspace = store
            .execute(update_profile_command(
                &source_project_id,
                &source_workspace,
                changed_source_profile,
            ))
            .response
            .business_workspace;
        assert_eq!(
            source_workspace.profile.customer_tax_id,
            "CUSTOMER-TAX-CHANGED"
        );
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
        assert_eq!(target_created.business_workspace, reopened_target);
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
            payments: Vec::new(),
            quote_confirmations: Vec::new(),
            receipts: Vec::new(),
            milestones: Vec::new(),
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
        assert_eq!(replayed.response.business_workspace, legacy_workspace);
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
}
