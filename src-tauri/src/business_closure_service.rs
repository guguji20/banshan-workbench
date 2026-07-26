use crate::asset_service;
use crate::protocol::{
    AssignBusinessCustomerPayload, AttachBusinessInvoiceAssetPayload, BusinessArchiveEntryRecord,
    BusinessArchiveIntegrityStatus, BusinessArchiveSnapshotRecord, BusinessArtifactRef,
    BusinessCustomerInput, BusinessCustomerRecord, BusinessCustomerStatus,
    BusinessDeliverableRecord, BusinessDeliverableVersionRecord, BusinessDeliverableVersionStatus,
    BusinessDeliverySignoffRecord, BusinessDeliverySubmissionRecord,
    BusinessDeliverySubmissionStatus, BusinessEvidenceInput, BusinessEvidenceKind,
    BusinessEvidenceRecord, BusinessInvoiceKind, BusinessInvoiceRecord, BusinessInvoiceStatus,
    BusinessMilestoneRecord, BusinessMilestoneStatus, BusinessWorkspaceRecord,
    CreateBusinessArchiveSnapshotPayload, HostError, RecordBusinessDeliverySentPayload,
    RecordBusinessDeliverySignoffPayload, RecordBusinessInvoiceIssuedPayload,
    RecordBusinessInvoiceRedCorrectionPayload, RegisterBusinessDeliverableVersionPayload,
    UpsertBusinessCustomerPayload, UpsertBusinessMilestonePayload,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAX_SHORT: usize = 240;
const MAX_TEXT: usize = 16_000;
const MAX_ITEMS: i64 = 5_000;
pub(crate) const BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE: &str = "BUSINESS_ARCHIVE_INTEGRITY_FAILED";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BusinessArchiveManifest {
    manifest_version: String,
    snapshot_id: String,
    workspace_id: String,
    project_id: String,
    captured_workspace_revision: i64,
    captured_customer_revision: i64,
    generated_by: String,
    generated_at: i64,
    entries: Vec<BusinessArchiveEntryRecord>,
}

pub(crate) fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS business_customers (
                id TEXT PRIMARY KEY NOT NULL,
                display_name TEXT NOT NULL,
                legal_name TEXT NOT NULL,
                tax_id TEXT NOT NULL,
                billing_address TEXT NOT NULL,
                primary_contact_name TEXT NOT NULL,
                primary_phone TEXT NOT NULL,
                primary_email TEXT NOT NULL,
                notes TEXT NOT NULL,
                display_name_key TEXT NOT NULL,
                legal_name_key TEXT NOT NULL,
                tax_id_key TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('active','archived')),
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                archived_at INTEGER,
                archived_by TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_business_customers_tax
                ON business_customers(tax_id_key)
                WHERE tax_id_key <> '' AND status = 'active';
            CREATE INDEX IF NOT EXISTS idx_business_customers_legal
                ON business_customers(legal_name_key, updated_at DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_business_customers_search
                ON business_customers(display_name_key, legal_name_key, tax_id_key);

            CREATE TABLE IF NOT EXISTS business_workspace_customers (
                workspace_id TEXT PRIMARY KEY NOT NULL,
                customer_id TEXT NOT NULL,
                match_kind TEXT NOT NULL CHECK(match_kind IN ('taxId','legalName','manual','unmatched')),
                linked_at INTEGER NOT NULL,
                linked_by TEXT NOT NULL,
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(customer_id) REFERENCES business_customers(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_workspace_customers_customer
                ON business_workspace_customers(customer_id, workspace_id);

            CREATE TABLE IF NOT EXISTS business_customer_conflicts (
                id TEXT PRIMARY KEY NOT NULL,
                left_customer_id TEXT NOT NULL,
                right_customer_id TEXT NOT NULL,
                match_key TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('pending','keptSeparate','merged')),
                reason TEXT NOT NULL,
                detected_at INTEGER NOT NULL,
                resolved_at INTEGER,
                resolved_by TEXT,
                CHECK(left_customer_id < right_customer_id),
                UNIQUE(left_customer_id, right_customer_id, match_key),
                FOREIGN KEY(left_customer_id) REFERENCES business_customers(id) ON DELETE RESTRICT,
                FOREIGN KEY(right_customer_id) REFERENCES business_customers(id) ON DELETE RESTRICT
            );

            CREATE TABLE IF NOT EXISTS business_customer_backfill (
                workspace_id TEXT PRIMARY KEY NOT NULL,
                customer_id TEXT NOT NULL,
                source_profile_sha256 TEXT NOT NULL CHECK(length(source_profile_sha256) = 64),
                completed_at INTEGER NOT NULL,
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(customer_id) REFERENCES business_customers(id) ON DELETE RESTRICT
            );

            CREATE TABLE IF NOT EXISTS business_delivery_milestones (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                sequence_number INTEGER NOT NULL CHECK(sequence_number >= 1),
                status TEXT NOT NULL CHECK(status IN ('planned','inProgress','delivered','accepted','canceled')),
                record_json TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(workspace_id, sequence_number),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_delivery_milestones_workspace
                ON business_delivery_milestones(workspace_id, sequence_number);

            CREATE TABLE IF NOT EXISTS business_deliverable_versions (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                milestone_id TEXT NOT NULL,
                deliverable_id TEXT NOT NULL,
                version_number INTEGER NOT NULL CHECK(version_number >= 1),
                status TEXT NOT NULL CHECK(status IN ('draft','sent','accepted','rejected','superseded')),
                asset_id TEXT NOT NULL,
                record_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE(deliverable_id, version_number),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(milestone_id) REFERENCES business_delivery_milestones(id) ON DELETE RESTRICT,
                FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_deliverable_versions_workspace
                ON business_deliverable_versions(workspace_id, milestone_id, deliverable_id, version_number);

            CREATE TABLE IF NOT EXISTS business_delivery_submissions (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                milestone_id TEXT NOT NULL,
                submission_number INTEGER NOT NULL CHECK(submission_number >= 1),
                status TEXT NOT NULL CHECK(status IN ('sent','partiallySigned','accepted','rejected')),
                record_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE(workspace_id, submission_number),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(milestone_id) REFERENCES business_delivery_milestones(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_delivery_submissions_workspace
                ON business_delivery_submissions(workspace_id, submission_number);

            CREATE TABLE IF NOT EXISTS business_invoices (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('issued','reversal')),
                invoice_identity TEXT NOT NULL UNIQUE,
                original_invoice_id TEXT,
                amount_cents INTEGER NOT NULL CHECK(amount_cents > 0),
                record_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                issued_at INTEGER NOT NULL,
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(original_invoice_id) REFERENCES business_invoices(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_invoices_workspace
                ON business_invoices(workspace_id, issued_at, id);

            CREATE TABLE IF NOT EXISTS business_archive_snapshots (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                captured_workspace_revision INTEGER NOT NULL CHECK(captured_workspace_revision >= 1),
                captured_customer_revision INTEGER NOT NULL CHECK(captured_customer_revision >= 1),
                manifest_sha256 TEXT NOT NULL CHECK(length(manifest_sha256) = 64),
                manifest_asset_id TEXT,
                package_asset_id TEXT,
                record_json TEXT NOT NULL,
                generated_at INTEGER NOT NULL,
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(manifest_asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(package_asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_archive_snapshots_workspace
                ON business_archive_snapshots(workspace_id, generated_at DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_business_archive_snapshots_manifest_asset
                ON business_archive_snapshots(manifest_asset_id);
            CREATE INDEX IF NOT EXISTS idx_business_archive_snapshots_package_asset
                ON business_archive_snapshots(package_asset_id);
            "#,
        )
        .map_err(sql_error)?;
    ensure_invoice_issued_at_column(connection)?;
    ensure_archive_asset_columns(connection)?;
    backfill_workspace_customers(connection)
}

fn ensure_invoice_issued_at_column(connection: &Connection) -> Result<(), HostError> {
    let exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('business_invoices') WHERE name = 'issued_at')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql_error)?;
    if !exists {
        connection
            .execute(
                "ALTER TABLE business_invoices ADD COLUMN issued_at INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

fn ensure_archive_asset_columns(connection: &Connection) -> Result<(), HostError> {
    for (column, definition) in [
        (
            "manifest_asset_id",
            "TEXT REFERENCES assets(id) ON DELETE RESTRICT",
        ),
        (
            "package_asset_id",
            "TEXT REFERENCES assets(id) ON DELETE RESTRICT",
        ),
    ] {
        let exists = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('business_archive_snapshots') WHERE name = ?1)",
                [column],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sql_error)?;
        if !exists {
            connection
                .execute(
                    &format!(
                        "ALTER TABLE business_archive_snapshots ADD COLUMN {column} {definition}"
                    ),
                    [],
                )
                .map_err(sql_error)?;
        }
    }
    connection
        .execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_business_archive_snapshots_manifest_asset
                 ON business_archive_snapshots(manifest_asset_id);
             CREATE INDEX IF NOT EXISTS idx_business_archive_snapshots_package_asset
                 ON business_archive_snapshots(package_asset_id);",
        )
        .map_err(sql_error)
}

fn backfill_workspace_customers(connection: &Connection) -> Result<(), HostError> {
    let rows = {
        let mut statement = connection
            .prepare(
                "SELECT w.id, w.profile_json, w.updated_at
                 FROM business_workspaces w
                 LEFT JOIN business_workspace_customers link ON link.workspace_id = w.id
                 WHERE link.workspace_id IS NULL
                 ORDER BY w.updated_at ASC, w.id ASC",
            )
            .map_err(sql_error)?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        collected
    };
    if rows.is_empty() {
        return Ok(());
    }
    connection
        .execute_batch("SAVEPOINT business_customer_backfill_v16")
        .map_err(sql_error)?;
    let result = (|| {
        for (workspace_id, profile_json, updated_at) in rows {
            let profile: crate::protocol::BusinessProfile =
                serde_json::from_str(&profile_json).map_err(json_error)?;
            let input = customer_input_from_profile(&profile);
            let (customer, match_kind) = resolve_or_create_customer(
                connection,
                &input,
                &workspace_id,
                "legacy-migration",
                updated_at,
            )?;
            connection
                .execute(
                    "INSERT INTO business_workspace_customers
                     (workspace_id, customer_id, match_kind, linked_at, linked_by)
                     VALUES (?1, ?2, ?3, ?4, 'legacy-migration')",
                    params![workspace_id, customer.id, match_kind, updated_at],
                )
                .map_err(sql_error)?;
            let profile_sha = format!("{:x}", Sha256::digest(profile_json.as_bytes()));
            connection
                .execute(
                    "INSERT OR IGNORE INTO business_customer_backfill
                     (workspace_id, customer_id, source_profile_sha256, completed_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![workspace_id, customer.id, profile_sha, updated_at],
                )
                .map_err(sql_error)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => connection
            .execute_batch("RELEASE SAVEPOINT business_customer_backfill_v16")
            .map_err(sql_error),
        Err(error) => {
            let _ = connection.execute_batch(
                "ROLLBACK TO SAVEPOINT business_customer_backfill_v16;
                 RELEASE SAVEPOINT business_customer_backfill_v16;",
            );
            Err(error)
        }
    }
}

pub(crate) fn attach_customer_for_new_workspace(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    profile: &crate::protocol::BusinessProfile,
    requested_customer_id: Option<&str>,
    actor_id: &str,
    now: i64,
) -> Result<BusinessCustomerRecord, HostError> {
    let (customer, match_kind) = if let Some(customer_id) = requested_customer_id {
        let customer = load_customer(transaction, customer_id)?;
        if customer.status != BusinessCustomerStatus::Active {
            return Err(HostError::new(
                "BUSINESS_CUSTOMER_ARCHIVED",
                "archived customer cannot be assigned to a new workspace",
                false,
            ));
        }
        (customer, "manual".to_string())
    } else {
        resolve_or_create_customer(
            transaction,
            &customer_input_from_profile(profile),
            workspace_id,
            actor_id,
            now,
        )?
    };
    transaction
        .execute(
            "INSERT INTO business_workspace_customers
             (workspace_id, customer_id, match_kind, linked_at, linked_by)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![workspace_id, customer.id, match_kind, now, actor_id],
        )
        .map_err(sql_error)?;
    Ok(customer)
}

pub(crate) fn load_customer_for_workspace(
    connection: &Connection,
    workspace_id: &str,
) -> Result<BusinessCustomerRecord, HostError> {
    let customer_id = connection
        .query_row(
            "SELECT customer_id FROM business_workspace_customers WHERE workspace_id = ?1",
            [workspace_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_CUSTOMER_NOT_LINKED",
                "business workspace has no customer master-data binding",
                false,
            )
        })?;
    load_customer(connection, &customer_id)
}

pub(crate) fn load_customer(
    connection: &Connection,
    customer_id: &str,
) -> Result<BusinessCustomerRecord, HostError> {
    connection
        .query_row(
            "SELECT id, display_name, legal_name, tax_id, billing_address,
                    primary_contact_name, primary_phone, primary_email, notes, status,
                    revision, created_at, updated_at, archived_at, archived_by
             FROM business_customers WHERE id = ?1",
            [customer_id],
            customer_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_CUSTOMER_NOT_FOUND",
                "business customer does not exist",
                false,
            )
        })
}

pub(crate) fn list_customers(
    connection: &Connection,
) -> Result<Vec<BusinessCustomerRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, display_name, legal_name, tax_id, billing_address,
                    primary_contact_name, primary_phone, primary_email, notes, status,
                    revision, created_at, updated_at, archived_at, archived_by
             FROM business_customers ORDER BY updated_at DESC, id DESC",
        )
        .map_err(sql_error)?;
    let records = statement
        .query_map([], customer_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

fn ensure_closure_row_updated(changed: usize, entity: &str) -> Result<(), HostError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_WORKSPACE_REVISION_CONFLICT",
            format!("{entity} was modified concurrently; reload and retry"),
            true,
        ))
    }
}

pub(crate) fn overlay_customer_profile(
    profile: &mut crate::protocol::BusinessProfile,
    customer: &BusinessCustomerRecord,
) {
    profile.customer_name = customer.display_name.clone();
    profile.customer_legal_name = customer.legal_name.clone();
    profile.customer_tax_id = customer.tax_id.clone();
    profile.customer_address = customer.billing_address.clone();
    profile.customer_contact = customer.primary_contact_name.clone();
    profile.customer_phone = customer.primary_phone.clone();
    profile.customer_email = customer.primary_email.clone();
}

pub(crate) fn upsert_customer(
    transaction: &Transaction<'_>,
    workspace: &BusinessWorkspaceRecord,
    payload: &UpsertBusinessCustomerPayload,
    actor_id: &str,
    now: i64,
) -> Result<(), HostError> {
    let input = normalize_customer_input(payload.customer.clone())?;
    let customer_id = payload
        .customer_id
        .as_deref()
        .unwrap_or(workspace.customer_id.as_str());
    let current = load_customer(transaction, customer_id)?;
    if current.status != BusinessCustomerStatus::Active {
        return Err(HostError::new(
            "BUSINESS_CUSTOMER_ARCHIVED",
            "archived customer cannot be edited",
            false,
        ));
    }
    ensure_customer_identity_available(transaction, customer_id, &input)?;
    let changed = transaction
        .execute(
            "UPDATE business_customers
             SET display_name = ?1, legal_name = ?2, tax_id = ?3, billing_address = ?4,
                 primary_contact_name = ?5, primary_phone = ?6, primary_email = ?7,
                 notes = ?8, display_name_key = ?9, legal_name_key = ?10,
                 tax_id_key = ?11, revision = revision + 1, updated_at = ?12
             WHERE id = ?13 AND revision = ?14",
            params![
                input.display_name,
                input.legal_name,
                input.tax_id,
                input.billing_address,
                input.primary_contact_name,
                input.primary_phone,
                input.primary_email,
                input.notes,
                normalized_name(&input.display_name),
                normalized_name(&input.legal_name),
                normalized_tax_id(&input.tax_id),
                now,
                current.id,
                current.revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_closure_row_updated(changed, "business customer")?;
    if customer_id != workspace.customer_id {
        ensure_workspace_customer_reassignable(workspace)?;
        assign_customer_id(transaction, workspace, customer_id, actor_id, now)?;
    }
    Ok(())
}

pub(crate) fn sync_customer_from_profile(
    transaction: &Transaction<'_>,
    workspace: &BusinessWorkspaceRecord,
    profile: &crate::protocol::BusinessProfile,
    now: i64,
) -> Result<(), HostError> {
    let input = customer_input_from_profile(profile);
    let current_tax = normalized_tax_id(&workspace.customer.tax_id);
    let next_tax = normalized_tax_id(&input.tax_id);
    let link_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM business_workspace_customers WHERE customer_id = ?1",
            [&workspace.customer_id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if !next_tax.is_empty() && next_tax != current_tax {
        // The profile now carries a different tax id. Two cases must redirect
        // the workspace binding instead of mutating the current customer
        // record in place: (a) the tax id already belongs to another customer
        // (in-place upsert would raise BUSINESS_CUSTOMER_IDENTITY_CONFLICT and
        // block the whole profile save forever), and (b) the current customer
        // record is shared with other workspaces (its identity must stay
        // intact). Redirecting is a customer rebind, so it honours the same
        // freeze rule as assignCustomer.
        let existing = find_customer_by_key(transaction, "tax_id_key", &next_tax)?;
        let owned_by_other = existing
            .as_ref()
            .is_some_and(|customer| customer.id != workspace.customer_id);
        if owned_by_other || link_count > 1 {
            ensure_workspace_customer_reassignable(workspace)?;
            let (customer, _) = resolve_or_create_customer(
                transaction,
                &input,
                &workspace.id,
                "workspace-profile-update",
                now,
            )?;
            transaction
                .execute(
                    "UPDATE business_workspace_customers
                     SET customer_id = ?1, match_kind = 'taxId', linked_at = ?2,
                         linked_by = 'workspace-profile-update'
                     WHERE workspace_id = ?3",
                    params![customer.id, now, workspace.id],
                )
                .map_err(sql_error)?;
            return Ok(());
        }
    }
    upsert_customer(
        transaction,
        workspace,
        &UpsertBusinessCustomerPayload {
            workspace_id: workspace.id.clone(),
            customer_id: Some(workspace.customer_id.clone()),
            customer: input,
        },
        "workspace-profile-update",
        now,
    )
}

pub(crate) fn assign_customer(
    transaction: &Transaction<'_>,
    workspace: &BusinessWorkspaceRecord,
    payload: &AssignBusinessCustomerPayload,
    actor_id: &str,
    now: i64,
) -> Result<(), HostError> {
    let customer = load_customer(transaction, &payload.customer_id)?;
    if customer.status != BusinessCustomerStatus::Active {
        return Err(HostError::new(
            "BUSINESS_CUSTOMER_ARCHIVED",
            "archived customer cannot be assigned",
            false,
        ));
    }
    ensure_workspace_customer_reassignable(workspace)?;
    assign_customer_id(transaction, workspace, &customer.id, actor_id, now)
}

fn assign_customer_id(
    transaction: &Transaction<'_>,
    workspace: &BusinessWorkspaceRecord,
    customer_id: &str,
    actor_id: &str,
    now: i64,
) -> Result<(), HostError> {
    if workspace.customer_id == customer_id {
        return Err(HostError::validation(
            "business workspace already uses requested customer",
        ));
    }
    transaction
        .execute(
            "UPDATE business_workspace_customers
             SET customer_id = ?1, match_kind = 'manual', linked_at = ?2, linked_by = ?3
             WHERE workspace_id = ?4",
            params![customer_id, now, actor_id, workspace.id],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn ensure_workspace_customer_reassignable(
    workspace: &BusinessWorkspaceRecord,
) -> Result<(), HostError> {
    if !workspace.documents.is_empty()
        || !workspace.payments.is_empty()
        || !workspace.quote_confirmations.is_empty()
        || !workspace.receipts.is_empty()
        || !workspace.milestones.is_empty()
        || !workspace.invoices.is_empty()
    {
        return Err(HostError::new(
            "BUSINESS_CUSTOMER_BINDING_FROZEN",
            "customer cannot be reassigned after the business ledger has started",
            false,
        ));
    }
    Ok(())
}

pub(crate) fn load_milestones(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessMilestoneRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT record_json FROM business_delivery_milestones
             WHERE workspace_id = ?1 ORDER BY sequence_number ASC, id ASC",
        )
        .map_err(sql_error)?;
    let mut milestones = statement
        .query_map([workspace_id], |row| {
            from_json_column(&row.get::<_, String>(0)?)
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<BusinessMilestoneRecord>, _>>()
        .map_err(sql_error)?;
    drop(statement);
    let versions = load_versions(connection, workspace_id)?;
    let mut grouped =
        HashMap::<String, BTreeMap<String, Vec<BusinessDeliverableVersionRecord>>>::new();
    for version in versions {
        grouped
            .entry(version.milestone_id.clone())
            .or_default()
            .entry(version.deliverable_id.clone())
            .or_default()
            .push(version);
    }
    for milestone in &mut milestones {
        let mut deliverables = Vec::new();
        if let Some(groups) = grouped.remove(&milestone.id) {
            for (deliverable_id, mut versions) in groups {
                versions.sort_by_key(|version| version.version_number);
                let latest = versions.last().expect("deliverable group is not empty");
                deliverables.push(BusinessDeliverableRecord {
                    id: deliverable_id,
                    milestone_id: milestone.id.clone(),
                    name: latest.name.clone(),
                    required: latest.required,
                    versions,
                });
            }
        }
        milestone.deliverables = deliverables;
    }
    Ok(milestones)
}

pub(crate) fn load_submissions(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessDeliverySubmissionRecord>, HostError> {
    load_json_records(
        connection,
        "SELECT record_json FROM business_delivery_submissions
         WHERE workspace_id = ?1 ORDER BY submission_number ASC, id ASC",
        workspace_id,
    )
}

pub(crate) fn load_invoices(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessInvoiceRecord>, HostError> {
    let mut invoices: Vec<BusinessInvoiceRecord> = load_json_records(
        connection,
        "SELECT record_json FROM business_invoices
         WHERE workspace_id = ?1 ORDER BY issued_at ASC, id ASC",
        workspace_id,
    )?;
    let mut reversed = HashMap::<String, i64>::new();
    for invoice in &invoices {
        if invoice.kind == BusinessInvoiceKind::Reversal {
            if let Some(original_id) = &invoice.original_invoice_id {
                *reversed.entry(original_id.clone()).or_default() += invoice.amount_cents;
            }
        }
    }
    for invoice in &mut invoices {
        if invoice.kind != BusinessInvoiceKind::Issued {
            continue;
        }
        let reversed_cents = reversed.get(&invoice.id).copied().unwrap_or_default();
        invoice.status = if reversed_cents <= 0 {
            BusinessInvoiceStatus::Issued
        } else if reversed_cents < invoice.amount_cents {
            BusinessInvoiceStatus::PartiallyReversed
        } else {
            BusinessInvoiceStatus::FullyReversed
        };
    }
    Ok(invoices)
}

pub(crate) fn load_archive_snapshots(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessArchiveSnapshotRecord>, HostError> {
    load_json_records(
        connection,
        "SELECT record_json FROM business_archive_snapshots
         WHERE workspace_id = ?1 ORDER BY generated_at ASC, id ASC",
        workspace_id,
    )
}

pub(crate) fn archive_integrity_status(
    workspace: &BusinessWorkspaceRecord,
) -> BusinessArchiveIntegrityStatus {
    let Some(snapshot) = workspace.archive_snapshots.last() else {
        return BusinessArchiveIntegrityStatus::NotCaptured;
    };
    if snapshot.captured_workspace_revision.saturating_add(1) == workspace.revision
        && snapshot.captured_customer_revision == workspace.customer.revision
    {
        BusinessArchiveIntegrityStatus::Ready
    } else {
        BusinessArchiveIntegrityStatus::Stale
    }
}

pub(crate) fn upsert_milestone(
    transaction: &Transaction<'_>,
    workspace: &BusinessWorkspaceRecord,
    payload: &UpsertBusinessMilestonePayload,
    now: i64,
) -> Result<(), HostError> {
    let input = &payload.milestone;
    validate_timestamp("dueAt", input.due_at)?;
    let title = required("title", &input.title, MAX_SHORT)?;
    let description = text("description", &input.description, MAX_TEXT)?;
    let acceptance_criteria = required("acceptanceCriteria", &input.acceptance_criteria, MAX_TEXT)?;
    if matches!(
        input.status,
        BusinessMilestoneStatus::Delivered | BusinessMilestoneStatus::Accepted
    ) {
        return Err(HostError::new(
            "BUSINESS_MILESTONE_STATUS_MANAGED",
            "delivered and accepted milestone states are managed by delivery commands",
            false,
        ));
    }
    if let Some(id) = &input.id {
        let current = workspace
            .milestones
            .iter()
            .find(|milestone| milestone.id == *id)
            .ok_or_else(|| {
                HostError::new(
                    "BUSINESS_MILESTONE_NOT_FOUND",
                    "delivery milestone does not exist",
                    false,
                )
            })?;
        if current.status == BusinessMilestoneStatus::Accepted {
            return Err(HostError::new(
                "BUSINESS_MILESTONE_ACCEPTED",
                "accepted delivery milestone is immutable",
                false,
            ));
        }
        let next = BusinessMilestoneRecord {
            id: current.id.clone(),
            sequence_number: current.sequence_number,
            title,
            description,
            due_at: input.due_at,
            acceptance_criteria,
            required: input.required,
            status: input.status.clone(),
            deliverables: Vec::new(),
            revision: current.revision + 1,
            created_at: current.created_at,
            updated_at: now,
        };
        let changed = transaction
            .execute(
                "UPDATE business_delivery_milestones
                 SET status = ?1, record_json = ?2, revision = revision + 1, updated_at = ?3
                 WHERE id = ?4 AND workspace_id = ?5 AND revision = ?6",
                params![
                    milestone_status_to_db(&next.status),
                    serde_json::to_string(&next).map_err(json_error)?,
                    now,
                    current.id,
                    workspace.id,
                    current.revision,
                ],
            )
            .map_err(sql_error)?;
        ensure_closure_row_updated(changed, "delivery milestone")?;
    } else {
        let count = workspace.milestones.len() as i64;
        if count >= MAX_ITEMS {
            return Err(HostError::new(
                "BUSINESS_MILESTONE_LIMIT_REACHED",
                "delivery milestone limit has been reached",
                false,
            ));
        }
        let record = BusinessMilestoneRecord {
            id: Uuid::new_v4().to_string(),
            sequence_number: count + 1,
            title,
            description,
            due_at: input.due_at,
            acceptance_criteria,
            required: input.required,
            status: input.status.clone(),
            deliverables: Vec::new(),
            revision: 1,
            created_at: now,
            updated_at: now,
        };
        transaction
            .execute(
                "INSERT INTO business_delivery_milestones
                 (id, workspace_id, sequence_number, status, record_json, revision, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
                params![
                    record.id,
                    workspace.id,
                    record.sequence_number,
                    milestone_status_to_db(&record.status),
                    serde_json::to_string(&record).map_err(json_error)?,
                    now,
                ],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

pub(crate) fn register_deliverable_version(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
    payload: &RegisterBusinessDeliverableVersionPayload,
    actor_id: &str,
    now: i64,
) -> Result<(), HostError> {
    let milestone = workspace
        .milestones
        .iter()
        .find(|milestone| milestone.id == payload.milestone_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_MILESTONE_NOT_FOUND",
                "delivery milestone does not exist",
                false,
            )
        })?;
    if matches!(
        milestone.status,
        BusinessMilestoneStatus::Accepted | BusinessMilestoneStatus::Canceled
    ) {
        return Err(HostError::new(
            "BUSINESS_MILESTONE_IMMUTABLE",
            "accepted or canceled milestone cannot receive another version",
            false,
        ));
    }
    let name = required("name", &payload.name, MAX_SHORT)?;
    let notes = text("notes", &payload.notes, MAX_TEXT)?;
    let artifact = verified_artifact_ref(
        transaction,
        vault_root,
        &workspace.project_id,
        &payload.asset_id,
        "delivery",
    )?;
    let deliverable_id = payload
        .deliverable_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let versions = load_versions(transaction, &workspace.id)?;
    let mut related = versions
        .into_iter()
        .filter(|version| version.deliverable_id == deliverable_id)
        .collect::<Vec<_>>();
    if related
        .iter()
        .any(|version| version.milestone_id != milestone.id)
    {
        return Err(HostError::new(
            "BUSINESS_DELIVERABLE_MILESTONE_CONFLICT",
            "deliverable belongs to another milestone",
            false,
        ));
    }
    for prior in related.iter_mut().filter(|version| {
        matches!(
            version.status,
            BusinessDeliverableVersionStatus::Draft
                | BusinessDeliverableVersionStatus::Sent
                | BusinessDeliverableVersionStatus::Rejected
        )
    }) {
        prior.status = BusinessDeliverableVersionStatus::Superseded;
        transaction
            .execute(
                "UPDATE business_deliverable_versions
                 SET status = 'superseded', record_json = ?1 WHERE id = ?2",
                params![serde_json::to_string(prior).map_err(json_error)?, prior.id],
            )
            .map_err(sql_error)?;
    }
    let version_number = related
        .iter()
        .map(|version| version.version_number)
        .max()
        .unwrap_or_default()
        + 1;
    let record = BusinessDeliverableVersionRecord {
        id: Uuid::new_v4().to_string(),
        deliverable_id,
        milestone_id: milestone.id.clone(),
        name,
        required: payload.required,
        version_number,
        artifact,
        status: BusinessDeliverableVersionStatus::Draft,
        notes,
        created_by: actor_id.to_string(),
        created_at: now,
    };
    transaction
        .execute(
            "INSERT INTO business_deliverable_versions
             (id, workspace_id, milestone_id, deliverable_id, version_number, status,
              asset_id, record_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'draft', ?6, ?7, ?8)",
            params![
                record.id,
                workspace.id,
                record.milestone_id,
                record.deliverable_id,
                record.version_number,
                record.artifact.asset_id,
                serde_json::to_string(&record).map_err(json_error)?,
                now,
            ],
        )
        .map_err(sql_error)?;
    set_milestone_system_status(
        transaction,
        milestone,
        BusinessMilestoneStatus::InProgress,
        now,
    )
}

pub(crate) fn record_delivery_sent(
    transaction: &Transaction<'_>,
    workspace: &BusinessWorkspaceRecord,
    payload: &RecordBusinessDeliverySentPayload,
    actor_id: &str,
    now: i64,
) -> Result<(), HostError> {
    validate_timestamp("sentAt", Some(payload.sent_at))?;
    let recipient = required("recipient", &payload.recipient, MAX_SHORT)?;
    let channel = required("channel", &payload.channel, MAX_SHORT)?;
    let note = text("note", &payload.note, MAX_TEXT)?;
    let version_ids = normalized_unique_ids("versionIds", &payload.version_ids)?;
    let milestone = workspace
        .milestones
        .iter()
        .find(|milestone| milestone.id == payload.milestone_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_MILESTONE_NOT_FOUND",
                "delivery milestone does not exist",
                false,
            )
        })?;
    let versions = load_versions(transaction, &workspace.id)?;
    let selected = versions
        .iter()
        .filter(|version| version_ids.contains(&version.id))
        .collect::<Vec<_>>();
    if selected.len() != version_ids.len()
        || selected.iter().any(|version| {
            version.milestone_id != milestone.id
                || version.status != BusinessDeliverableVersionStatus::Draft
        })
    {
        return Err(HostError::new(
            "BUSINESS_DELIVERY_VERSION_INVALID",
            "every sent version must be a draft in the selected milestone",
            false,
        ));
    }
    for version in selected {
        let mut next = version.clone();
        next.status = BusinessDeliverableVersionStatus::Sent;
        transaction
            .execute(
                "UPDATE business_deliverable_versions SET status = 'sent', record_json = ?1 WHERE id = ?2",
                params![serde_json::to_string(&next).map_err(json_error)?, version.id],
            )
            .map_err(sql_error)?;
    }
    let submission_number: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(submission_number), 0) + 1
             FROM business_delivery_submissions WHERE workspace_id = ?1",
            [&workspace.id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    let record = BusinessDeliverySubmissionRecord {
        id: Uuid::new_v4().to_string(),
        milestone_id: milestone.id.clone(),
        submission_number,
        version_ids: version_ids.into_iter().collect(),
        recipient,
        channel,
        note,
        sent_at: payload.sent_at,
        sent_by: actor_id.to_string(),
        status: BusinessDeliverySubmissionStatus::Sent,
        signoffs: Vec::new(),
    };
    transaction
        .execute(
            "INSERT INTO business_delivery_submissions
             (id, workspace_id, milestone_id, submission_number, status, record_json, created_at)
             VALUES (?1, ?2, ?3, ?4, 'sent', ?5, ?6)",
            params![
                record.id,
                workspace.id,
                record.milestone_id,
                record.submission_number,
                serde_json::to_string(&record).map_err(json_error)?,
                now,
            ],
        )
        .map_err(sql_error)?;
    set_milestone_system_status(
        transaction,
        milestone,
        BusinessMilestoneStatus::Delivered,
        now,
    )
}

pub(crate) fn record_delivery_signoff(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
    payload: &RecordBusinessDeliverySignoffPayload,
    actor_id: &str,
    now: i64,
) -> Result<(), HostError> {
    validate_timestamp("occurredAt", Some(payload.occurred_at))?;
    let representative = required(
        "customerRepresentative",
        &payload.customer_representative,
        MAX_SHORT,
    )?;
    let note = text("note", &payload.note, MAX_TEXT)?;
    let accepted = normalized_ids_allow_empty("acceptedVersionIds", &payload.accepted_version_ids)?;
    let rejected = normalized_ids_allow_empty("rejectedVersionIds", &payload.rejected_version_ids)?;
    if accepted.intersection(&rejected).next().is_some() {
        return Err(HostError::validation(
            "acceptedVersionIds and rejectedVersionIds must not overlap",
        ));
    }
    let mut submission = workspace
        .delivery_submissions
        .iter()
        .find(|submission| submission.id == payload.submission_id)
        .cloned()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_DELIVERY_SUBMISSION_NOT_FOUND",
                "delivery submission does not exist",
                false,
            )
        })?;
    let submitted = submission
        .version_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut previously_accepted = BTreeSet::new();
    let mut previously_rejected = BTreeSet::new();
    for signoff in &submission.signoffs {
        previously_accepted.extend(signoff.accepted_version_ids.iter().cloned());
        previously_rejected.extend(signoff.rejected_version_ids.iter().cloned());
    }
    let previously_decided = previously_accepted
        .union(&previously_rejected)
        .cloned()
        .collect::<BTreeSet<_>>();
    if previously_decided.len() >= submitted.len() {
        // Every version in this batch already carries a decision; further
        // deliveries continue in a new submission.
        return Err(HostError::new(
            "BUSINESS_DELIVERY_ALREADY_SIGNED",
            "every version in this delivery submission already has a signoff decision",
            false,
        ));
    }
    let decided = accepted.union(&rejected).cloned().collect::<BTreeSet<_>>();
    if decided.is_empty() || !decided.is_subset(&submitted) {
        return Err(HostError::validation(
            "signoff decisions must reference versions in the submission",
        ));
    }
    if decided.intersection(&previously_decided).next().is_some() {
        // Follow-up signoffs may only cover versions that are still pending;
        // recorded decisions are immutable evidence.
        return Err(HostError::new(
            "BUSINESS_DELIVERY_ALREADY_SIGNED",
            "some versions in this signoff already have a recorded decision",
            false,
        ));
    }
    let evidence = payload
        .evidence
        .as_ref()
        .map(|input| {
            evidence_record(
                transaction,
                vault_root,
                &workspace.project_id,
                BusinessEvidenceKind::AcceptanceProof,
                input,
                actor_id,
                now,
            )
        })
        .transpose()?;
    let versions = load_versions(transaction, &workspace.id)?;
    for version in versions
        .iter()
        .filter(|version| decided.contains(&version.id))
    {
        let mut next = version.clone();
        next.status = if accepted.contains(&version.id) {
            BusinessDeliverableVersionStatus::Accepted
        } else {
            BusinessDeliverableVersionStatus::Rejected
        };
        transaction
            .execute(
                "UPDATE business_deliverable_versions SET status = ?1, record_json = ?2 WHERE id = ?3",
                params![
                    deliverable_status_to_db(&next.status),
                    serde_json::to_string(&next).map_err(json_error)?,
                    next.id,
                ],
            )
            .map_err(sql_error)?;
    }
    let signoff = BusinessDeliverySignoffRecord {
        id: Uuid::new_v4().to_string(),
        submission_id: submission.id.clone(),
        accepted_version_ids: accepted.iter().cloned().collect(),
        rejected_version_ids: rejected.iter().cloned().collect(),
        customer_representative: representative,
        evidence,
        note,
        occurred_at: payload.occurred_at,
        recorded_by: actor_id.to_string(),
        recorded_at: now,
    };
    let total_accepted = previously_accepted
        .union(&accepted)
        .cloned()
        .collect::<BTreeSet<_>>();
    let total_rejected = previously_rejected
        .union(&rejected)
        .cloned()
        .collect::<BTreeSet<_>>();
    let total_decided = total_accepted
        .union(&total_rejected)
        .cloned()
        .collect::<BTreeSet<_>>();
    submission.status = if total_rejected.is_empty() && total_decided == submitted {
        BusinessDeliverySubmissionStatus::Accepted
    } else if total_accepted.is_empty() && total_decided == submitted {
        BusinessDeliverySubmissionStatus::Rejected
    } else {
        BusinessDeliverySubmissionStatus::PartiallySigned
    };
    submission.signoffs.push(signoff);
    transaction
        .execute(
            "UPDATE business_delivery_submissions SET status = ?1, record_json = ?2 WHERE id = ?3",
            params![
                submission_status_to_db(&submission.status),
                serde_json::to_string(&submission).map_err(json_error)?,
                submission.id,
            ],
        )
        .map_err(sql_error)?;
    let milestone = workspace
        .milestones
        .iter()
        .find(|milestone| milestone.id == submission.milestone_id)
        .ok_or_else(|| HostError::internal("delivery submission milestone is missing"))?;
    let status = if submission.status == BusinessDeliverySubmissionStatus::Accepted {
        BusinessMilestoneStatus::Accepted
    } else {
        BusinessMilestoneStatus::InProgress
    };
    set_milestone_system_status(transaction, milestone, status, now)
}

pub(crate) fn record_invoice_issued(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
    payload: &RecordBusinessInvoiceIssuedPayload,
    actor_id: &str,
    now: i64,
) -> Result<(), HostError> {
    validate_invoice_amounts(payload.amount_cents, payload.tax_cents)?;
    validate_timestamp("issuedAt", Some(payload.issued_at))?;
    let code = required("invoiceCode", &payload.invoice_code, MAX_SHORT)?;
    let number = required("invoiceNumber", &payload.invoice_number, MAX_SHORT)?;
    if let Some(payment_id) = &payload.payment_id {
        if !workspace
            .payments
            .iter()
            .any(|payment| payment.id == *payment_id)
        {
            return Err(HostError::new(
                "BUSINESS_PAYMENT_NOT_FOUND",
                "invoice payment does not belong to this workspace",
                false,
            ));
        }
    }
    let artifacts = verified_artifact_refs(
        transaction,
        vault_root,
        &workspace.project_id,
        &payload.asset_ids,
        "invoice",
    )?;
    if artifacts.is_empty() {
        return Err(HostError::new(
            "BUSINESS_INVOICE_ATTACHMENT_REQUIRED",
            "issued invoice requires at least one Vault attachment",
            false,
        ));
    }
    let net_before = invoice_net_cents(&workspace.invoices);
    if workspace.financial_summary.contract_cents <= 0
        || net_before.saturating_add(payload.amount_cents)
            > workspace.financial_summary.contract_cents
    {
        return Err(HostError::new(
            "BUSINESS_INVOICE_EXCEEDS_CONTRACT",
            "issued invoice total cannot exceed current contract amount",
            false,
        ));
    }
    let record = BusinessInvoiceRecord {
        id: Uuid::new_v4().to_string(),
        payment_id: payload.payment_id.clone(),
        kind: BusinessInvoiceKind::Issued,
        status: BusinessInvoiceStatus::Issued,
        invoice_code: code,
        invoice_number: number,
        issuer_tax_id: workspace.profile.supplier_tax_id.clone(),
        buyer_tax_id: workspace.customer.tax_id.clone(),
        currency: workspace.profile.currency.clone(),
        amount_cents: payload.amount_cents,
        tax_cents: payload.tax_cents,
        issued_at: payload.issued_at,
        original_invoice_id: None,
        reversal_reason: String::new(),
        artifacts,
        recorded_by: actor_id.to_string(),
        created_at: now,
    };
    insert_invoice(transaction, workspace, &record)
}

pub(crate) fn record_invoice_red_correction(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
    payload: &RecordBusinessInvoiceRedCorrectionPayload,
    actor_id: &str,
    now: i64,
) -> Result<(), HostError> {
    validate_invoice_amounts(payload.amount_cents, payload.tax_cents)?;
    validate_timestamp("issuedAt", Some(payload.issued_at))?;
    let reason = required("reason", &payload.reason, MAX_TEXT)?;
    let original = workspace
        .invoices
        .iter()
        .find(|invoice| {
            invoice.id == payload.original_invoice_id && invoice.kind == BusinessInvoiceKind::Issued
        })
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_INVOICE_NOT_FOUND",
                "original issued invoice does not exist",
                false,
            )
        })?;
    let already_reversed = workspace
        .invoices
        .iter()
        .filter(|invoice| {
            invoice.kind == BusinessInvoiceKind::Reversal
                && invoice.original_invoice_id.as_deref() == Some(original.id.as_str())
        })
        .map(|invoice| invoice.amount_cents)
        .sum::<i64>();
    if payload.amount_cents > original.amount_cents.saturating_sub(already_reversed) {
        return Err(HostError::new(
            "BUSINESS_INVOICE_REVERSAL_EXCEEDS_REMAINING",
            "red correction exceeds the remaining reversible invoice amount",
            false,
        ));
    }
    let artifacts = verified_artifact_refs(
        transaction,
        vault_root,
        &workspace.project_id,
        &payload.asset_ids,
        "invoiceReversal",
    )?;
    if artifacts.is_empty() {
        return Err(HostError::new(
            "BUSINESS_INVOICE_ATTACHMENT_REQUIRED",
            "red correction requires a red-invoice attachment",
            false,
        ));
    }
    let record = BusinessInvoiceRecord {
        id: Uuid::new_v4().to_string(),
        payment_id: original.payment_id.clone(),
        kind: BusinessInvoiceKind::Reversal,
        status: BusinessInvoiceStatus::Issued,
        invoice_code: required("invoiceCode", &payload.invoice_code, MAX_SHORT)?,
        invoice_number: required("invoiceNumber", &payload.invoice_number, MAX_SHORT)?,
        issuer_tax_id: original.issuer_tax_id.clone(),
        buyer_tax_id: original.buyer_tax_id.clone(),
        currency: original.currency.clone(),
        amount_cents: payload.amount_cents,
        tax_cents: payload.tax_cents,
        issued_at: payload.issued_at,
        original_invoice_id: Some(original.id.clone()),
        reversal_reason: reason,
        artifacts,
        recorded_by: actor_id.to_string(),
        created_at: now,
    };
    insert_invoice(transaction, workspace, &record)
}

pub(crate) fn attach_invoice_asset(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
    payload: &AttachBusinessInvoiceAssetPayload,
) -> Result<(), HostError> {
    let mut invoice = workspace
        .invoices
        .iter()
        .find(|invoice| invoice.id == payload.invoice_id)
        .cloned()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_INVOICE_NOT_FOUND",
                "business invoice does not exist",
                false,
            )
        })?;
    let role = required("role", &payload.role, MAX_SHORT)?;
    let artifact = verified_artifact_ref(
        transaction,
        vault_root,
        &workspace.project_id,
        &payload.asset_id,
        &role,
    )?;
    if invoice
        .artifacts
        .iter()
        .any(|existing| existing.asset_id == artifact.asset_id)
    {
        return Err(HostError::validation(
            "invoice already contains requested attachment",
        ));
    }
    invoice.artifacts.push(artifact);
    transaction
        .execute(
            "UPDATE business_invoices SET record_json = ?1 WHERE id = ?2 AND workspace_id = ?3",
            params![
                serde_json::to_string(&invoice).map_err(json_error)?,
                invoice.id,
                workspace.id,
            ],
        )
        .map_err(sql_error)?;
    Ok(())
}

pub(crate) struct PreparedBusinessArchiveSnapshot {
    pub(crate) snapshot: BusinessArchiveSnapshotRecord,
    pub(crate) manifest_path: PathBuf,
    pub(crate) package_path: PathBuf,
    staging_dir: PathBuf,
}

impl Drop for PreparedBusinessArchiveSnapshot {
    fn drop(&mut self) {
        let expected_parent = std::env::temp_dir().join("bsaigc-business-archive");
        if self.staging_dir.starts_with(&expected_parent) {
            let _ = fs::remove_dir_all(&self.staging_dir);
        }
    }
}

pub(crate) fn prepare_archive_snapshot(
    connection: &Connection,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
    _payload: &CreateBusinessArchiveSnapshotPayload,
    snapshot_id: &str,
    actor_id: &str,
    now: i64,
) -> Result<PreparedBusinessArchiveSnapshot, HostError> {
    let snapshot_id = Uuid::parse_str(snapshot_id)
        .map(|value| value.to_string())
        .map_err(|_| HostError::validation("archive snapshotId must be a UUID"))?;
    let entries = collect_archive_entries(connection, vault_root, workspace)?;
    if entries.is_empty() {
        return Err(HostError::new(
            "BUSINESS_ARCHIVE_EMPTY",
            "archive snapshot requires at least one authoritative asset",
            false,
        ));
    }
    let manifest = BusinessArchiveManifest {
        manifest_version: "business-archive.v1".to_string(),
        snapshot_id: snapshot_id.clone(),
        workspace_id: workspace.id.clone(),
        project_id: workspace.project_id.clone(),
        captured_workspace_revision: workspace.revision,
        captured_customer_revision: workspace.customer.revision,
        generated_by: actor_id.to_string(),
        generated_at: now,
        entries: entries.clone(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(json_error)?;
    let manifest_sha256 = format!("{:x}", Sha256::digest(&manifest_bytes));
    let staging_parent = std::env::temp_dir().join("bsaigc-business-archive");
    fs::create_dir_all(&staging_parent)
        .map_err(|error| archive_io_error("create archive staging parent", error))?;
    let staging_dir = staging_parent.join(format!("{}-{}", snapshot_id, Uuid::new_v4()));
    fs::create_dir(&staging_dir)
        .map_err(|error| archive_io_error("create archive staging directory", error))?;
    let manifest_path = staging_dir.join("manifest.json");
    let package_path = staging_dir.join("archive.zip");
    let prepared = (|| {
        let mut manifest_file = File::create(&manifest_path)
            .map_err(|error| archive_io_error("create archive manifest", error))?;
        manifest_file
            .write_all(&manifest_bytes)
            .map_err(|error| archive_io_error("write archive manifest", error))?;
        manifest_file
            .sync_all()
            .map_err(|error| archive_io_error("sync archive manifest", error))?;
        write_archive_package(
            connection,
            vault_root,
            &workspace.project_id,
            &entries,
            &manifest_bytes,
            &package_path,
        )?;
        Ok(PreparedBusinessArchiveSnapshot {
            snapshot: BusinessArchiveSnapshotRecord {
                id: snapshot_id,
                captured_workspace_revision: workspace.revision,
                captured_customer_revision: workspace.customer.revision,
                manifest_sha256,
                manifest_asset_id: None,
                package_asset_id: None,
                entries,
                generated_by: actor_id.to_string(),
                generated_at: now,
            },
            manifest_path,
            package_path,
            staging_dir: staging_dir.clone(),
        })
    })();
    if prepared.is_err() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    prepared
}

pub(crate) fn persist_archive_snapshot(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
    snapshot: &BusinessArchiveSnapshotRecord,
) -> Result<(), HostError> {
    if snapshot.captured_workspace_revision != workspace.revision
        || snapshot.captured_customer_revision != workspace.customer.revision
    {
        return Err(HostError::conflict(
            "business workspace or customer changed while the archive package was being generated",
        ));
    }
    let manifest_asset_id = snapshot
        .manifest_asset_id
        .as_deref()
        .ok_or_else(|| HostError::internal("archive snapshot is missing its manifest Asset"))?;
    let package_asset_id = snapshot
        .package_asset_id
        .as_deref()
        .ok_or_else(|| HostError::internal("archive snapshot is missing its package Asset"))?;
    let manifest_artifact = verified_artifact_ref(
        transaction,
        vault_root,
        &workspace.project_id,
        manifest_asset_id,
        "archiveManifest",
    )?;
    if manifest_artifact.sha256 != snapshot.manifest_sha256 {
        return Err(HostError::new(
            "BUSINESS_ARCHIVE_MANIFEST_MISMATCH",
            "archive manifest Asset does not match the captured manifest digest",
            false,
        ));
    }
    verified_artifact_ref(
        transaction,
        vault_root,
        &workspace.project_id,
        package_asset_id,
        "archivePackage",
    )?;
    transaction
        .execute(
            "INSERT INTO business_archive_snapshots
             (id, workspace_id, captured_workspace_revision, captured_customer_revision,
              manifest_sha256, manifest_asset_id, package_asset_id, record_json, generated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                snapshot.id,
                workspace.id,
                snapshot.captured_workspace_revision,
                snapshot.captured_customer_revision,
                snapshot.manifest_sha256,
                snapshot.manifest_asset_id,
                snapshot.package_asset_id,
                serde_json::to_string(snapshot).map_err(json_error)?,
                snapshot.generated_at,
            ],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn write_archive_package(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    entries: &[BusinessArchiveEntryRecord],
    manifest_bytes: &[u8],
    package_path: &Path,
) -> Result<(), HostError> {
    let file = File::create(package_path)
        .map_err(|error| archive_io_error("create archive package", error))?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    archive
        .start_file("manifest.json", options)
        .map_err(|error| {
            HostError::internal(format!("start archive manifest entry failed: {error}"))
        })?;
    archive
        .write_all(manifest_bytes)
        .map_err(|error| archive_io_error("write archive manifest entry", error))?;

    let mut buffer = vec![0_u8; 1024 * 1024];
    for entry in entries {
        let (asset, source_path) = asset_service::verify_ready_asset_integrity(
            connection,
            vault_root,
            &entry.artifact.asset_id,
        )?;
        if asset.project_id.as_deref() != Some(project_id)
            || asset.sha256 != entry.artifact.sha256
            || asset.size_bytes != entry.artifact.size_bytes
        {
            return Err(HostError::new(
                "BUSINESS_ARCHIVE_ENTRY_MISMATCH",
                "archive entry changed after manifest capture",
                false,
            ));
        }
        archive
            .start_file(&entry.logical_path, options)
            .map_err(|error| {
                HostError::internal(format!("start archive asset entry failed: {error}"))
            })?;
        let mut source = File::open(&source_path)
            .map_err(|error| archive_io_error("open archive source asset", error))?;
        let mut hasher = Sha256::new();
        let mut observed_size = 0_i64;
        loop {
            let read = source
                .read(&mut buffer)
                .map_err(|error| archive_io_error("read archive source asset", error))?;
            if read == 0 {
                break;
            }
            archive
                .write_all(&buffer[..read])
                .map_err(|error| archive_io_error("write archive source asset", error))?;
            hasher.update(&buffer[..read]);
            observed_size = observed_size
                .checked_add(read as i64)
                .ok_or_else(|| HostError::internal("archive entry size overflow"))?;
        }
        if observed_size != entry.artifact.size_bytes
            || format!("{:x}", hasher.finalize()) != entry.artifact.sha256
        {
            return Err(HostError::new(
                "BUSINESS_ARCHIVE_ENTRY_MISMATCH",
                "archive entry changed while the package was being written",
                false,
            ));
        }
    }
    let output = archive
        .finish()
        .map_err(|error| HostError::internal(format!("finish archive package failed: {error}")))?;
    output
        .sync_all()
        .map_err(|error| archive_io_error("sync archive package", error))
}

fn archive_io_error(action: &str, error: std::io::Error) -> HostError {
    HostError::new(
        "BUSINESS_ARCHIVE_IO",
        format!("{action} failed: {error}"),
        true,
    )
}

/// Verifies the durable archive artifacts and every byte represented by a snapshot.
/// This is intentionally independent from revision freshness so callers can distinguish
/// a stale-but-intact snapshot from a corrupt snapshot before archive or export actions.
pub(crate) fn verify_archive_snapshot_integrity(
    connection: &Connection,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
    snapshot: &BusinessArchiveSnapshotRecord,
) -> Result<(), HostError> {
    verify_archive_snapshot_integrity_for_workspace(
        connection,
        vault_root,
        &workspace.id,
        &workspace.project_id,
        snapshot,
    )
}

fn verify_archive_snapshot_integrity_for_workspace(
    connection: &Connection,
    vault_root: &Path,
    workspace_id: &str,
    project_id: &str,
    snapshot: &BusinessArchiveSnapshotRecord,
) -> Result<(), HostError> {
    let manifest_asset_id = snapshot
        .manifest_asset_id
        .as_deref()
        .ok_or_else(|| archive_integrity_error("archive snapshot is missing its manifest Asset"))?;
    let package_asset_id = snapshot
        .package_asset_id
        .as_deref()
        .ok_or_else(|| archive_integrity_error("archive snapshot is missing its package Asset"))?;
    if manifest_asset_id == package_asset_id {
        return Err(archive_integrity_error(
            "archive manifest and package must be distinct Assets",
        ));
    }

    let (manifest_asset, manifest_path) = verify_archive_asset(
        connection,
        vault_root,
        project_id,
        manifest_asset_id,
        "manifest",
    )?;
    if manifest_asset.sha256 != snapshot.manifest_sha256 {
        return Err(archive_integrity_error(
            "archive manifest digest does not match the snapshot",
        ));
    }
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|_| archive_integrity_error("archive manifest Vault file cannot be read"))?;
    if manifest_bytes.len() as u64 != manifest_asset.size_bytes as u64
        || format!("{:x}", Sha256::digest(&manifest_bytes)) != manifest_asset.sha256
    {
        return Err(archive_integrity_error(
            "archive manifest changed during integrity verification",
        ));
    }
    let manifest: BusinessArchiveManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| {
            archive_integrity_error("archive manifest is not valid business archive JSON")
        })?;
    let expected_manifest = BusinessArchiveManifest {
        manifest_version: "business-archive.v1".to_string(),
        snapshot_id: snapshot.id.clone(),
        workspace_id: workspace_id.to_string(),
        project_id: project_id.to_string(),
        captured_workspace_revision: snapshot.captured_workspace_revision,
        captured_customer_revision: snapshot.captured_customer_revision,
        generated_by: snapshot.generated_by.clone(),
        generated_at: snapshot.generated_at,
        entries: snapshot.entries.clone(),
    };
    if manifest != expected_manifest {
        return Err(archive_integrity_error(
            "archive manifest fields or entries do not match the snapshot",
        ));
    }

    let mut expected_entries = BTreeMap::<String, &BusinessArchiveEntryRecord>::new();
    for entry in &snapshot.entries {
        if entry.logical_path == "manifest.json"
            || entry.logical_path.is_empty()
            || entry.logical_path.ends_with('/')
            || entry.artifact.size_bytes < 0
            || entry.artifact.sha256.len() != 64
            || expected_entries
                .insert(entry.logical_path.clone(), entry)
                .is_some()
        {
            return Err(archive_integrity_error(
                "archive snapshot contains invalid or duplicate entry metadata",
            ));
        }
    }

    let (package_asset, package_path) = verify_archive_asset(
        connection,
        vault_root,
        project_id,
        package_asset_id,
        "package",
    )?;
    let package_file = File::open(&package_path)
        .map_err(|_| archive_integrity_error("archive package Vault file cannot be opened"))?;
    let mut archive = ZipArchive::new(package_file)
        .map_err(|_| archive_integrity_error("archive package is not a readable ZIP"))?;
    let mut seen = BTreeSet::<String>::new();
    let mut manifest_seen = false;
    for index in 0..archive.len() {
        let mut zip_entry = archive
            .by_index(index)
            .map_err(|_| archive_integrity_error("archive package entry cannot be opened"))?;
        let logical_path = zip_entry.name().to_string();
        if zip_entry.is_dir() || !seen.insert(logical_path.clone()) {
            return Err(archive_integrity_error(
                "archive package contains a directory or duplicate entry",
            ));
        }
        if logical_path == "manifest.json" {
            manifest_seen = true;
            if zip_entry.size() != manifest_bytes.len() as u64 {
                return Err(archive_integrity_error(
                    "ZIP manifest size does not match the external manifest",
                ));
            }
            let mut packaged_manifest = Vec::with_capacity(manifest_bytes.len());
            zip_entry
                .read_to_end(&mut packaged_manifest)
                .map_err(|_| archive_integrity_error("ZIP manifest cannot be read"))?;
            if packaged_manifest != manifest_bytes {
                return Err(archive_integrity_error(
                    "ZIP manifest does not match the external manifest",
                ));
            }
            continue;
        }
        let expected = expected_entries.get(&logical_path).ok_or_else(|| {
            archive_integrity_error("archive package contains an unexpected entry")
        })?;
        verify_zip_entry(&mut zip_entry, expected)?;
    }
    if !manifest_seen || seen.len() != expected_entries.len().saturating_add(1) {
        return Err(archive_integrity_error(
            "archive package entry set does not match the snapshot",
        ));
    }
    if expected_entries
        .keys()
        .any(|logical_path| !seen.contains(logical_path))
    {
        return Err(archive_integrity_error(
            "archive package is missing a snapshot entry",
        ));
    }

    let (final_manifest_asset, _) = verify_archive_asset(
        connection,
        vault_root,
        project_id,
        manifest_asset_id,
        "manifest",
    )?;
    let (final_package_asset, _) = verify_archive_asset(
        connection,
        vault_root,
        project_id,
        package_asset_id,
        "package",
    )?;
    if final_manifest_asset.sha256 != manifest_asset.sha256
        || final_manifest_asset.size_bytes != manifest_asset.size_bytes
        || final_package_asset.sha256 != package_asset.sha256
        || final_package_asset.size_bytes != package_asset.size_bytes
    {
        return Err(archive_integrity_error(
            "archive Assets changed during integrity verification",
        ));
    }
    Ok(())
}

fn verify_archive_asset(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    asset_id: &str,
    label: &str,
) -> Result<(crate::protocol::AssetRecord, PathBuf), HostError> {
    let (asset, path) = asset_service::verify_ready_asset_integrity(
        connection, vault_root, asset_id,
    )
    .map_err(|_| {
        archive_integrity_error(format!(
            "archive {label} Asset record or Vault bytes failed verification"
        ))
    })?;
    if asset.project_id.as_deref() != Some(project_id) {
        return Err(archive_integrity_error(format!(
            "archive {label} Asset belongs to another project"
        )));
    }
    Ok((asset, path))
}

fn verify_zip_entry(
    zip_entry: &mut impl Read,
    expected: &BusinessArchiveEntryRecord,
) -> Result<(), HostError> {
    let expected_size = u64::try_from(expected.artifact.size_bytes).map_err(|_| {
        archive_integrity_error("archive snapshot entry has an invalid expected size")
    })?;
    let mut hasher = Sha256::new();
    let mut observed_size = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = zip_entry
            .read(&mut buffer)
            .map_err(|_| archive_integrity_error("archive package entry cannot be read"))?;
        if read == 0 {
            break;
        }
        observed_size = observed_size
            .checked_add(read as u64)
            .ok_or_else(|| archive_integrity_error("archive package entry size overflowed"))?;
        if observed_size > expected_size {
            return Err(archive_integrity_error(
                "archive package entry is larger than the snapshot metadata",
            ));
        }
        hasher.update(&buffer[..read]);
    }
    if observed_size != expected_size
        || format!("{:x}", hasher.finalize()) != expected.artifact.sha256
    {
        return Err(archive_integrity_error(
            "archive package entry size or digest does not match the snapshot",
        ));
    }
    Ok(())
}

fn archive_integrity_error(message: impl Into<String>) -> HostError {
    HostError::new(BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE, message, false)
}

pub(crate) fn ensure_closure_archivable(
    workspace: &BusinessWorkspaceRecord,
) -> Result<(), HostError> {
    let mut blockers = Vec::new();
    let required = workspace
        .milestones
        .iter()
        .filter(|milestone| milestone.required)
        .collect::<Vec<_>>();
    if required.is_empty() {
        blockers.push("required delivery milestone");
    }
    if required
        .iter()
        .any(|milestone| milestone.status != BusinessMilestoneStatus::Accepted)
    {
        blockers.push("all required delivery milestones must be accepted");
    }
    if required.iter().any(|milestone| {
        milestone.deliverables.iter().any(|deliverable| {
            deliverable.required
                && !deliverable
                    .versions
                    .iter()
                    .any(|version| version.status == BusinessDeliverableVersionStatus::Accepted)
        })
    }) {
        blockers.push("all required deliverables need an accepted version");
    }
    if workspace.delivery_submissions.iter().any(|submission| {
        // A batch blocks archiving only while versions are still awaiting a
        // decision. Fully-decided batches (including rejected ones) are
        // closed history — the milestone-acceptance blocker above already
        // guarantees a later batch delivered the accepted result.
        let decided = submission
            .signoffs
            .iter()
            .flat_map(|signoff| {
                signoff
                    .accepted_version_ids
                    .iter()
                    .chain(signoff.rejected_version_ids.iter())
            })
            .collect::<BTreeSet<_>>();
        submission
            .version_ids
            .iter()
            .any(|version_id| !decided.contains(version_id))
    }) {
        blockers.push("delivery submissions must be fully decided");
    }
    if workspace.invoices.is_empty()
        || invoice_net_cents(&workspace.invoices) != workspace.financial_summary.contract_cents
    {
        blockers.push("invoice net amount must equal the effective contract amount");
    }
    if workspace
        .invoices
        .iter()
        .any(|invoice| invoice.artifacts.is_empty())
    {
        blockers.push("every invoice and red correction needs a Vault attachment");
    }
    if workspace.archive_integrity_status != BusinessArchiveIntegrityStatus::Ready {
        blockers.push("fresh archive integrity snapshot");
    }
    if blockers.is_empty() {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_CLOSURE_ARCHIVE_BLOCKED",
            format!("business closure is incomplete: {}", blockers.join(", ")),
            false,
        ))
    }
}

fn collect_archive_entries(
    connection: &Connection,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
) -> Result<Vec<BusinessArchiveEntryRecord>, HostError> {
    let mut sources = BTreeMap::<String, (String, String, String)>::new();
    let mut add = |asset_id: &Option<String>, role: &str, entity_type: &str, entity_id: &str| {
        if let Some(asset_id) = asset_id.as_ref().filter(|value| !value.is_empty()) {
            sources.entry(asset_id.clone()).or_insert_with(|| {
                (
                    role.to_string(),
                    entity_type.to_string(),
                    entity_id.to_string(),
                )
            });
        }
    };
    for document in &workspace.documents {
        add(
            &document.output_asset_id,
            "document",
            "businessDocument",
            &document.id,
        );
        add(
            &document.source_asset_id,
            "signedContract",
            "businessDocument",
            &document.id,
        );
        add(
            &document.report_asset_id,
            "reviewReport",
            "businessDocument",
            &document.id,
        );
        if let Some(evidence) = &document.evidence {
            add(
                &Some(evidence.asset_id.clone()),
                "documentEvidence",
                "businessDocument",
                &document.id,
            );
        }
    }
    for confirmation in &workspace.quote_confirmations {
        add(
            &Some(confirmation.quote_asset_id.clone()),
            "confirmedQuote",
            "quoteConfirmation",
            &confirmation.id,
        );
        add(
            &Some(confirmation.evidence.asset_id.clone()),
            "quoteEvidence",
            "quoteConfirmation",
            &confirmation.id,
        );
    }
    for receipt in &workspace.receipts {
        if let Some(evidence) = &receipt.evidence {
            add(
                &Some(evidence.asset_id.clone()),
                "receiptEvidence",
                "receipt",
                &receipt.id,
            );
        }
    }
    for milestone in &workspace.milestones {
        for deliverable in &milestone.deliverables {
            for version in &deliverable.versions {
                add(
                    &Some(version.artifact.asset_id.clone()),
                    "deliverable",
                    "deliverableVersion",
                    &version.id,
                );
            }
        }
    }
    for submission in &workspace.delivery_submissions {
        for signoff in &submission.signoffs {
            if let Some(evidence) = &signoff.evidence {
                add(
                    &Some(evidence.asset_id.clone()),
                    "deliverySignoff",
                    "deliverySubmission",
                    &submission.id,
                );
            }
        }
    }
    for invoice in &workspace.invoices {
        for artifact in &invoice.artifacts {
            add(
                &Some(artifact.asset_id.clone()),
                &artifact.role,
                "invoice",
                &invoice.id,
            );
        }
    }
    let mut entries = Vec::with_capacity(sources.len());
    for (index, (asset_id, (role, entity_type, entity_id))) in sources.into_iter().enumerate() {
        let artifact = verified_artifact_ref(
            connection,
            vault_root,
            &workspace.project_id,
            &asset_id,
            &role,
        )?;
        let safe_name = artifact.original_name.replace(['/', '\\'], "_");
        entries.push(BusinessArchiveEntryRecord {
            logical_path: format!("assets/{:04}-{}", index + 1, safe_name),
            role,
            source_entity_type: entity_type,
            source_entity_id: entity_id,
            artifact,
        });
    }
    Ok(entries)
}

fn resolve_or_create_customer(
    connection: &Connection,
    input: &BusinessCustomerInput,
    fallback_id: &str,
    actor_id: &str,
    now: i64,
) -> Result<(BusinessCustomerRecord, String), HostError> {
    let input = normalize_customer_input(input.clone())?;
    let tax_key = normalized_tax_id(&input.tax_id);
    let legal_key = normalized_name(&input.legal_name);
    if !tax_key.is_empty() {
        if let Some(customer) = find_customer_by_key(connection, "tax_id_key", &tax_key)? {
            return Ok((customer, "taxId".to_string()));
        }
    } else if !legal_key.is_empty() {
        let matches = find_customers_by_legal_key(connection, &legal_key)?;
        if matches.len() == 1 {
            return Ok((matches[0].clone(), "legalName".to_string()));
        }
    }
    let identity = if !tax_key.is_empty() {
        format!("tax:{tax_key}")
    } else if !legal_key.is_empty() {
        format!("legal:{legal_key}")
    } else {
        format!("workspace:{fallback_id}")
    };
    let customer_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()).to_string();
    let record = BusinessCustomerRecord {
        id: customer_id.clone(),
        display_name: input.display_name.clone(),
        legal_name: input.legal_name.clone(),
        tax_id: input.tax_id.clone(),
        billing_address: input.billing_address.clone(),
        primary_contact_name: input.primary_contact_name.clone(),
        primary_phone: input.primary_phone.clone(),
        primary_email: input.primary_email.clone(),
        notes: input.notes.clone(),
        status: BusinessCustomerStatus::Active,
        revision: 1,
        created_at: now,
        updated_at: now,
        archived_at: None,
        archived_by: None,
    };
    connection
        .execute(
            "INSERT OR IGNORE INTO business_customers
             (id, display_name, legal_name, tax_id, billing_address, primary_contact_name,
              primary_phone, primary_email, notes, display_name_key, legal_name_key,
              tax_id_key, status, revision, created_at, updated_at, archived_at, archived_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                     'active', 1, ?13, ?13, NULL, NULL)",
            params![
                record.id,
                record.display_name,
                record.legal_name,
                record.tax_id,
                record.billing_address,
                record.primary_contact_name,
                record.primary_phone,
                record.primary_email,
                record.notes,
                normalized_name(&record.display_name),
                normalized_name(&record.legal_name),
                normalized_tax_id(&record.tax_id),
                now,
            ],
        )
        .map_err(sql_error)?;
    let stored = load_customer(connection, &customer_id)?;
    if tax_key.is_empty() && !legal_key.is_empty() {
        let conflicts = find_customers_by_legal_key(connection, &legal_key)?;
        for other in conflicts.into_iter().filter(|other| other.id != stored.id) {
            let (left, right) = if stored.id < other.id {
                (stored.id.clone(), other.id)
            } else {
                (other.id, stored.id.clone())
            };
            connection
                .execute(
                    "INSERT OR IGNORE INTO business_customer_conflicts
                     (id, left_customer_id, right_customer_id, match_key, status, reason,
                      detected_at, resolved_at, resolved_by)
                     VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?6, NULL, NULL)",
                    params![
                        Uuid::new_v5(
                            &Uuid::NAMESPACE_OID,
                            format!("{left}:{right}:{legal_key}").as_bytes(),
                        )
                        .to_string(),
                        left,
                        right,
                        legal_key,
                        format!("legacy legal-name ambiguity detected by {actor_id}"),
                        now,
                    ],
                )
                .map_err(sql_error)?;
        }
    }
    let match_kind = if !tax_key.is_empty() {
        "taxId"
    } else if !legal_key.is_empty() {
        "legalName"
    } else {
        "unmatched"
    };
    Ok((stored, match_kind.to_string()))
}

fn customer_input_from_profile(
    profile: &crate::protocol::BusinessProfile,
) -> BusinessCustomerInput {
    BusinessCustomerInput {
        display_name: profile.customer_name.clone(),
        legal_name: profile.customer_legal_name.clone(),
        tax_id: profile.customer_tax_id.clone(),
        billing_address: profile.customer_address.clone(),
        primary_contact_name: profile.customer_contact.clone(),
        primary_phone: profile.customer_phone.clone(),
        primary_email: profile.customer_email.clone(),
        notes: String::new(),
    }
}

fn normalize_customer_input(
    input: BusinessCustomerInput,
) -> Result<BusinessCustomerInput, HostError> {
    let display_name = text("displayName", &input.display_name, MAX_SHORT)?;
    let legal_name = text("legalName", &input.legal_name, MAX_SHORT)?;
    if display_name.is_empty() && legal_name.is_empty() {
        return Err(HostError::validation(
            "customer displayName or legalName is required",
        ));
    }
    Ok(BusinessCustomerInput {
        display_name,
        legal_name,
        tax_id: text("taxId", &input.tax_id, MAX_SHORT)?,
        billing_address: text("billingAddress", &input.billing_address, MAX_TEXT)?,
        primary_contact_name: text("primaryContactName", &input.primary_contact_name, MAX_SHORT)?,
        primary_phone: text("primaryPhone", &input.primary_phone, MAX_SHORT)?,
        primary_email: text("primaryEmail", &input.primary_email, MAX_SHORT)?,
        notes: text("notes", &input.notes, MAX_TEXT)?,
    })
}

fn ensure_customer_identity_available(
    connection: &Connection,
    customer_id: &str,
    input: &BusinessCustomerInput,
) -> Result<(), HostError> {
    let tax_key = normalized_tax_id(&input.tax_id);
    if !tax_key.is_empty() {
        if let Some(existing) = find_customer_by_key(connection, "tax_id_key", &tax_key)? {
            if existing.id != customer_id {
                return Err(HostError::new(
                    "BUSINESS_CUSTOMER_IDENTITY_CONFLICT",
                    format!("taxId already belongs to customer {}", existing.id),
                    false,
                ));
            }
        }
    }
    Ok(())
}

fn find_customer_by_key(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<BusinessCustomerRecord>, HostError> {
    let sql = format!(
        "SELECT id, display_name, legal_name, tax_id, billing_address,
                primary_contact_name, primary_phone, primary_email, notes, status,
                revision, created_at, updated_at, archived_at, archived_by
         FROM business_customers WHERE {column} = ?1 AND status = 'active'
         ORDER BY updated_at DESC, id DESC LIMIT 1"
    );
    connection
        .query_row(&sql, [value], customer_from_row)
        .optional()
        .map_err(sql_error)
}

fn find_customers_by_legal_key(
    connection: &Connection,
    legal_key: &str,
) -> Result<Vec<BusinessCustomerRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, display_name, legal_name, tax_id, billing_address,
                    primary_contact_name, primary_phone, primary_email, notes, status,
                    revision, created_at, updated_at, archived_at, archived_by
             FROM business_customers WHERE legal_name_key = ?1 AND status = 'active'
             ORDER BY updated_at DESC, id DESC",
        )
        .map_err(sql_error)?;
    let records = statement
        .query_map([legal_key], customer_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

fn customer_from_row(row: &Row<'_>) -> rusqlite::Result<BusinessCustomerRecord> {
    let status: String = row.get(9)?;
    Ok(BusinessCustomerRecord {
        id: row.get(0)?,
        display_name: row.get(1)?,
        legal_name: row.get(2)?,
        tax_id: row.get(3)?,
        billing_address: row.get(4)?,
        primary_contact_name: row.get(5)?,
        primary_phone: row.get(6)?,
        primary_email: row.get(7)?,
        notes: row.get(8)?,
        status: match status.as_str() {
            "active" => BusinessCustomerStatus::Active,
            "archived" => BusinessCustomerStatus::Archived,
            _ => return Err(conversion_error("business customer status", &status)),
        },
        revision: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        archived_at: row.get(13)?,
        archived_by: row.get(14)?,
    })
}

fn load_versions(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessDeliverableVersionRecord>, HostError> {
    load_json_records(
        connection,
        "SELECT record_json FROM business_deliverable_versions
         WHERE workspace_id = ?1 ORDER BY milestone_id, deliverable_id, version_number",
        workspace_id,
    )
}

fn load_json_records<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    sql: &str,
    workspace_id: &str,
) -> Result<Vec<T>, HostError> {
    let mut statement = connection.prepare(sql).map_err(sql_error)?;
    let records = statement
        .query_map([workspace_id], |row| {
            from_json_column(&row.get::<_, String>(0)?)
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

fn set_milestone_system_status(
    transaction: &Transaction<'_>,
    milestone: &BusinessMilestoneRecord,
    status: BusinessMilestoneStatus,
    now: i64,
) -> Result<(), HostError> {
    let mut next = milestone.clone();
    next.status = status;
    next.revision += 1;
    next.updated_at = now;
    next.deliverables.clear();
    let changed = transaction
        .execute(
            "UPDATE business_delivery_milestones
             SET status = ?1, record_json = ?2, revision = revision + 1, updated_at = ?3
             WHERE id = ?4 AND revision = ?5",
            params![
                milestone_status_to_db(&next.status),
                serde_json::to_string(&next).map_err(json_error)?,
                now,
                milestone.id,
                milestone.revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_closure_row_updated(changed, "delivery milestone")?;
    Ok(())
}

fn evidence_record(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    kind: BusinessEvidenceKind,
    input: &BusinessEvidenceInput,
    actor_id: &str,
    now: i64,
) -> Result<BusinessEvidenceRecord, HostError> {
    let artifact = verified_artifact_ref(
        connection,
        vault_root,
        project_id,
        &input.asset_id,
        "evidence",
    )?;
    Ok(BusinessEvidenceRecord {
        kind,
        asset_id: artifact.asset_id,
        sha256: artifact.sha256,
        occurred_at: input.occurred_at,
        note: text("evidence.note", &input.note, MAX_TEXT)?,
        recorded_by: actor_id.to_string(),
        recorded_at: now,
    })
}

fn verified_artifact_refs(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    asset_ids: &[String],
    role: &str,
) -> Result<Vec<BusinessArtifactRef>, HostError> {
    let ids = normalized_unique_ids("assetIds", asset_ids)?;
    ids.into_iter()
        .map(|id| verified_artifact_ref(connection, vault_root, project_id, &id, role))
        .collect()
}

fn verified_artifact_ref(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    asset_id: &str,
    role: &str,
) -> Result<BusinessArtifactRef, HostError> {
    let (asset, _) = asset_service::verify_ready_asset_integrity(connection, vault_root, asset_id)?;
    if asset.project_id.as_deref() != Some(project_id) {
        return Err(HostError::new(
            "BUSINESS_ASSET_PROJECT_MISMATCH",
            "business artifact must belong to the workspace project",
            false,
        ));
    }
    Ok(BusinessArtifactRef {
        role: role.to_string(),
        asset_id: asset.id,
        sha256: asset.sha256,
        size_bytes: asset.size_bytes,
        original_name: asset.original_name,
    })
}

fn insert_invoice(
    transaction: &Transaction<'_>,
    workspace: &BusinessWorkspaceRecord,
    record: &BusinessInvoiceRecord,
) -> Result<(), HostError> {
    let identity = format!(
        "{}:{}:{}",
        normalized_tax_id(&record.issuer_tax_id),
        normalized_name(&record.invoice_code),
        normalized_name(&record.invoice_number)
    );
    transaction
        .execute(
            "INSERT INTO business_invoices
             (id, workspace_id, kind, invoice_identity, original_invoice_id,
              amount_cents, record_json, created_at, issued_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id,
                workspace.id,
                invoice_kind_to_db(&record.kind),
                identity,
                record.original_invoice_id,
                record.amount_cents,
                serde_json::to_string(record).map_err(json_error)?,
                record.created_at,
                record.issued_at,
            ],
        )
        .map_err(|error| {
            if is_unique_violation(&error) {
                HostError::new(
                    "BUSINESS_INVOICE_DUPLICATE",
                    "issuer, invoice code and invoice number already exist",
                    false,
                )
            } else {
                sql_error(error)
            }
        })?;
    Ok(())
}

fn invoice_net_cents(invoices: &[BusinessInvoiceRecord]) -> i64 {
    invoices
        .iter()
        .fold(0_i64, |total, invoice| match invoice.kind {
            BusinessInvoiceKind::Issued => total.saturating_add(invoice.amount_cents),
            BusinessInvoiceKind::Reversal => total.saturating_sub(invoice.amount_cents),
        })
}

fn validate_invoice_amounts(amount_cents: i64, tax_cents: i64) -> Result<(), HostError> {
    if amount_cents <= 0 || tax_cents < 0 || tax_cents > amount_cents {
        return Err(HostError::validation(
            "invoice amountCents must be positive and taxCents must be within the amount",
        ));
    }
    Ok(())
}

fn normalized_unique_ids(field: &str, values: &[String]) -> Result<BTreeSet<String>, HostError> {
    if values.is_empty() {
        return Err(HostError::validation(format!("{field} must not be empty")));
    }
    let mut result = BTreeSet::new();
    for value in values {
        let normalized = Uuid::parse_str(value.trim())
            .map_err(|_| HostError::validation(format!("{field} must contain UUID values")))?
            .to_string();
        if !result.insert(normalized) {
            return Err(HostError::validation(format!(
                "{field} must not contain duplicates"
            )));
        }
    }
    Ok(result)
}

fn normalized_ids_allow_empty(
    field: &str,
    values: &[String],
) -> Result<BTreeSet<String>, HostError> {
    if values.is_empty() {
        return Ok(BTreeSet::new());
    }
    normalized_unique_ids(field, values)
}

fn required(field: &str, value: &str, max: usize) -> Result<String, HostError> {
    let value = text(field, value, max)?;
    if value.is_empty() {
        Err(HostError::validation(format!("{field} is required")))
    } else {
        Ok(value)
    }
}

fn text(field: &str, value: &str, max: usize) -> Result<String, HostError> {
    let value = value.trim().to_string();
    if value.chars().count() > max || value.chars().any(|character| character.is_control()) {
        Err(HostError::validation(format!(
            "{field} must contain at most {max} visible characters"
        )))
    } else {
        Ok(value)
    }
}

fn validate_timestamp(field: &str, value: Option<i64>) -> Result<(), HostError> {
    if value.is_some_and(|value| value < 0) {
        Err(HostError::validation(format!(
            "{field} must be non-negative"
        )))
    } else {
        Ok(())
    }
}

fn normalized_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalized_tax_id(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(|character| character.to_uppercase())
        .collect()
}

fn milestone_status_to_db(status: &BusinessMilestoneStatus) -> &'static str {
    match status {
        BusinessMilestoneStatus::Planned => "planned",
        BusinessMilestoneStatus::InProgress => "inProgress",
        BusinessMilestoneStatus::Delivered => "delivered",
        BusinessMilestoneStatus::Accepted => "accepted",
        BusinessMilestoneStatus::Canceled => "canceled",
    }
}

fn deliverable_status_to_db(status: &BusinessDeliverableVersionStatus) -> &'static str {
    match status {
        BusinessDeliverableVersionStatus::Draft => "draft",
        BusinessDeliverableVersionStatus::Sent => "sent",
        BusinessDeliverableVersionStatus::Accepted => "accepted",
        BusinessDeliverableVersionStatus::Rejected => "rejected",
        BusinessDeliverableVersionStatus::Superseded => "superseded",
    }
}

fn submission_status_to_db(status: &BusinessDeliverySubmissionStatus) -> &'static str {
    match status {
        BusinessDeliverySubmissionStatus::Sent => "sent",
        BusinessDeliverySubmissionStatus::PartiallySigned => "partiallySigned",
        BusinessDeliverySubmissionStatus::Accepted => "accepted",
        BusinessDeliverySubmissionStatus::Rejected => "rejected",
    }
}

fn invoice_kind_to_db(kind: &BusinessInvoiceKind) -> &'static str {
    match kind {
        BusinessInvoiceKind::Issued => "issued",
        BusinessInvoiceKind::Reversal => "reversal",
    }
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(code, _)
            if code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                || code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
    )
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

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::new("SQLITE_ERROR", error.to_string(), true)
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::new("JSON_ERROR", error.to_string(), false)
}

#[cfg(test)]
mod archive_integrity_tests {
    use super::*;
    use crate::protocol::BusinessArtifactRef;

    struct ArchiveIntegrityFixture {
        temporary: tempfile::TempDir,
        connection: Connection,
        vault_root: PathBuf,
        workspace_id: String,
        project_id: String,
        snapshot: BusinessArchiveSnapshotRecord,
        manifest_bytes: Vec<u8>,
        entry_bytes: Vec<u8>,
    }

    impl ArchiveIntegrityFixture {
        fn new() -> Self {
            let temporary = tempfile::tempdir().expect("create archive fixture directory");
            let vault_root = temporary.path().join("vault");
            fs::create_dir_all(&vault_root).expect("create fixture Vault");
            let mut connection = Connection::open_in_memory().expect("open fixture database");
            asset_service::migrate(&connection).expect("migrate Asset schema");
            let workspace_id = Uuid::new_v4().to_string();
            let project_id = Uuid::new_v4().to_string();
            let entry_bytes = b"signed contract bytes".to_vec();
            let source_path = temporary.path().join("signed-contract.pdf");
            fs::write(&source_path, &entry_bytes).expect("write source fixture");
            let source_asset = asset_service::import_file(
                &mut connection,
                &vault_root,
                Some(&project_id),
                &source_path,
            )
            .expect("import source fixture");
            let entry = BusinessArchiveEntryRecord {
                logical_path: "assets/0001-signed-contract.pdf".to_string(),
                role: "contract".to_string(),
                source_entity_type: "businessDocument".to_string(),
                source_entity_id: Uuid::new_v4().to_string(),
                artifact: BusinessArtifactRef {
                    role: "contract".to_string(),
                    asset_id: source_asset.id,
                    sha256: source_asset.sha256,
                    size_bytes: source_asset.size_bytes,
                    original_name: source_asset.original_name,
                },
            };
            let mut snapshot = BusinessArchiveSnapshotRecord {
                id: Uuid::new_v4().to_string(),
                captured_workspace_revision: 17,
                captured_customer_revision: 4,
                manifest_sha256: String::new(),
                manifest_asset_id: None,
                package_asset_id: None,
                entries: vec![entry],
                generated_by: "operator-1".to_string(),
                generated_at: 1_700_000_000_000,
            };
            let manifest = manifest_for(&workspace_id, &project_id, &snapshot);
            let manifest_bytes =
                serde_json::to_vec_pretty(&manifest).expect("serialize fixture manifest");
            let manifest_asset = import_fixture_asset(
                &temporary,
                &mut connection,
                &vault_root,
                &project_id,
                "manifest.json",
                &manifest_bytes,
            );
            let package_path = temporary.path().join("archive.zip");
            write_fixture_package(
                &package_path,
                &manifest_bytes,
                &[(&snapshot.entries[0].logical_path, entry_bytes.as_slice())],
            );
            let package_asset = asset_service::import_file(
                &mut connection,
                &vault_root,
                Some(&project_id),
                &package_path,
            )
            .expect("import fixture package");
            snapshot.manifest_sha256 = manifest_asset.sha256;
            snapshot.manifest_asset_id = Some(manifest_asset.id);
            snapshot.package_asset_id = Some(package_asset.id);
            Self {
                temporary,
                connection,
                vault_root,
                workspace_id,
                project_id,
                snapshot,
                manifest_bytes,
                entry_bytes,
            }
        }

        fn verify(&self) -> Result<(), HostError> {
            verify_archive_snapshot_integrity_for_workspace(
                &self.connection,
                &self.vault_root,
                &self.workspace_id,
                &self.project_id,
                &self.snapshot,
            )
        }

        fn asset_path(&self, asset_id: &str) -> PathBuf {
            asset_service::resolve_original_path(&self.connection, &self.vault_root, asset_id)
                .expect("resolve fixture Asset path")
        }

        fn delete_asset_bytes(&self, asset_id: &str) {
            fs::remove_file(self.asset_path(asset_id)).expect("delete fixture Asset bytes");
        }

        fn corrupt_asset_bytes(&self, asset_id: &str) {
            let path = self.asset_path(asset_id);
            let mut bytes = fs::read(&path).expect("read fixture Asset bytes");
            assert!(!bytes.is_empty());
            let index = bytes.len() / 2;
            bytes[index] ^= 0x01;
            fs::write(path, bytes).expect("corrupt fixture Asset bytes");
        }

        fn replace_manifest(&mut self, manifest: &BusinessArchiveManifest) {
            let bytes =
                serde_json::to_vec_pretty(manifest).expect("serialize replacement manifest");
            let asset = import_fixture_asset(
                &self.temporary,
                &mut self.connection,
                &self.vault_root,
                &self.project_id,
                "replacement-manifest.json",
                &bytes,
            );
            self.snapshot.manifest_sha256 = asset.sha256;
            self.snapshot.manifest_asset_id = Some(asset.id);
            self.manifest_bytes = bytes;
        }

        fn replace_package(&mut self, manifest_bytes: &[u8], entries: &[(&str, &[u8])]) {
            let package_path = self
                .temporary
                .path()
                .join(format!("replacement-{}.zip", Uuid::new_v4()));
            write_fixture_package(&package_path, manifest_bytes, entries);
            let asset = asset_service::import_file(
                &mut self.connection,
                &self.vault_root,
                Some(&self.project_id),
                &package_path,
            )
            .expect("import replacement package");
            self.snapshot.package_asset_id = Some(asset.id);
        }
    }

    fn manifest_for(
        workspace_id: &str,
        project_id: &str,
        snapshot: &BusinessArchiveSnapshotRecord,
    ) -> BusinessArchiveManifest {
        BusinessArchiveManifest {
            manifest_version: "business-archive.v1".to_string(),
            snapshot_id: snapshot.id.clone(),
            workspace_id: workspace_id.to_string(),
            project_id: project_id.to_string(),
            captured_workspace_revision: snapshot.captured_workspace_revision,
            captured_customer_revision: snapshot.captured_customer_revision,
            generated_by: snapshot.generated_by.clone(),
            generated_at: snapshot.generated_at,
            entries: snapshot.entries.clone(),
        }
    }

    fn import_fixture_asset(
        temporary: &tempfile::TempDir,
        connection: &mut Connection,
        vault_root: &Path,
        project_id: &str,
        file_name: &str,
        bytes: &[u8],
    ) -> crate::protocol::AssetRecord {
        let path = temporary
            .path()
            .join(format!("{}-{file_name}", Uuid::new_v4()));
        fs::write(&path, bytes).expect("write fixture Asset source");
        asset_service::import_file(connection, vault_root, Some(project_id), &path)
            .expect("import fixture Asset")
    }

    fn write_fixture_package(path: &Path, manifest_bytes: &[u8], entries: &[(&str, &[u8])]) {
        let file = File::create(path).expect("create fixture ZIP");
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        archive
            .start_file("manifest.json", options)
            .expect("start fixture manifest entry");
        archive
            .write_all(manifest_bytes)
            .expect("write fixture manifest entry");
        for (logical_path, bytes) in entries {
            archive
                .start_file(*logical_path, options)
                .expect("start fixture archive entry");
            archive
                .write_all(bytes)
                .expect("write fixture archive entry");
        }
        let output = archive.finish().expect("finish fixture ZIP");
        output.sync_all().expect("sync fixture ZIP");
    }

    fn assert_integrity_error(result: Result<(), HostError>) {
        let error = result.expect_err("archive verification must fail");
        assert_eq!(error.code, BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE);
        assert!(!error.retryable);
    }

    #[test]
    fn archive_snapshot_integrity_accepts_complete_archive() {
        let fixture = ArchiveIntegrityFixture::new();
        fixture.verify().expect("verify complete archive");
    }

    #[test]
    fn archive_snapshot_integrity_rejects_deleted_manifest() {
        let fixture = ArchiveIntegrityFixture::new();
        let manifest_asset_id = fixture.snapshot.manifest_asset_id.clone().unwrap();
        fixture.delete_asset_bytes(&manifest_asset_id);
        assert_integrity_error(fixture.verify());
    }

    #[test]
    fn archive_snapshot_integrity_rejects_tampered_manifest() {
        let fixture = ArchiveIntegrityFixture::new();
        let manifest_asset_id = fixture.snapshot.manifest_asset_id.clone().unwrap();
        fixture.corrupt_asset_bytes(&manifest_asset_id);
        assert_integrity_error(fixture.verify());
    }

    #[test]
    fn archive_snapshot_integrity_rejects_deleted_package() {
        let fixture = ArchiveIntegrityFixture::new();
        let package_asset_id = fixture.snapshot.package_asset_id.clone().unwrap();
        fixture.delete_asset_bytes(&package_asset_id);
        assert_integrity_error(fixture.verify());
    }

    #[test]
    fn archive_snapshot_integrity_rejects_tampered_package() {
        let fixture = ArchiveIntegrityFixture::new();
        let package_asset_id = fixture.snapshot.package_asset_id.clone().unwrap();
        fixture.corrupt_asset_bytes(&package_asset_id);
        assert_integrity_error(fixture.verify());
    }

    #[test]
    fn archive_snapshot_integrity_rejects_manifest_fields_that_differ_from_snapshot() {
        let mut fixture = ArchiveIntegrityFixture::new();
        let mut manifest = manifest_for(
            &fixture.workspace_id,
            &fixture.project_id,
            &fixture.snapshot,
        );
        manifest.generated_by = "different-operator".to_string();
        fixture.replace_manifest(&manifest);
        assert_integrity_error(fixture.verify());
    }

    #[test]
    fn archive_snapshot_integrity_rejects_zip_manifest_that_differs_from_external_manifest() {
        let mut fixture = ArchiveIntegrityFixture::new();
        let mut packaged_manifest = fixture.manifest_bytes.clone();
        packaged_manifest.push(b' ');
        let logical_path = fixture.snapshot.entries[0].logical_path.clone();
        let entry_bytes = fixture.entry_bytes.clone();
        fixture.replace_package(
            &packaged_manifest,
            &[(logical_path.as_str(), entry_bytes.as_slice())],
        );
        assert_integrity_error(fixture.verify());
    }

    #[test]
    fn archive_snapshot_integrity_rejects_zip_entry_digest_mismatch() {
        let mut fixture = ArchiveIntegrityFixture::new();
        let manifest_bytes = fixture.manifest_bytes.clone();
        let logical_path = fixture.snapshot.entries[0].logical_path.clone();
        let mut tampered_entry = fixture.entry_bytes.clone();
        tampered_entry[0] ^= 0x01;
        fixture.replace_package(
            &manifest_bytes,
            &[(logical_path.as_str(), tampered_entry.as_slice())],
        );
        assert_integrity_error(fixture.verify());
    }

    #[test]
    fn archive_snapshot_integrity_rejects_missing_zip_entry() {
        let mut fixture = ArchiveIntegrityFixture::new();
        let manifest_bytes = fixture.manifest_bytes.clone();
        fixture.replace_package(&manifest_bytes, &[]);
        assert_integrity_error(fixture.verify());
    }
}
