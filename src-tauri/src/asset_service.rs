use crate::protocol::{
    is_legacy_surface_protocol_supported, AssetCommandEnvelope, AssetCommandResponse,
    AssetDomainEvent, AssetEventType, AssetKind, AssetRecord, AssetStatus, CommandReceipt,
    HostError, LEGACY_PROTOCOL_VERSION, PREVIOUS_PROTOCOL_VERSION, PROTOCOL_1_3_VERSION,
    PROTOCOL_VERSION,
};
use fs2::FileExt;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const SNIFF_BYTES: usize = 512;
const IMPORT_RECOVERY_DIRECTORY: &str = ".asset-import-recovery";
const IMPORT_RECOVERY_LOCK_FILE: &str = ".reconcile.lock";
const IMPORT_RECOVERY_INTENT_SUFFIX: &str = ".intent";
const IMPORT_RECOVERY_INTENT_VERSION: u32 = 1;
const MAX_IMPORT_RECOVERY_INTENT_BYTES: u64 = 8 * 1024;
const ASSET_ORIGIN_USER: &str = "user";
const ASSET_ORIGIN_BUSINESS_DOCUMENT: &str = "businessDocument";
const ASSET_ORIGIN_GENERATED_EXTRACTION_SNAPSHOT: &str = "generatedExtractionSnapshot";
const ASSET_ORIGIN_GENERATED_REVIEW_REPORT: &str = "generatedReviewReport";
const ASSET_ORIGIN_GENERATED_PAGE_PREVIEW: &str = "generatedPagePreview";
const ASSET_ORIGIN_GENERATED_ARCHIVE_MANIFEST: &str = "generatedArchiveManifest";
const ASSET_ORIGIN_GENERATED_ARCHIVE_PACKAGE: &str = "generatedArchivePackage";
const ASSET_ORIGIN_NORMALIZED_TEMPLATE: &str = "normalizedTemplate";
const MAX_GENERATED_ARTIFACT_REF_CHARS: usize = 256;
const MAX_TEMPLATE_ASSET_SIZE_BYTES: u64 = 64 * 1024 * 1024;

/// A local safety ceiling against accidentally importing devices or enormous
/// sparse files. Normal production video files remain well below this limit.
pub const MAX_ASSET_SIZE_BYTES: u64 = 1024 * 1024 * 1024 * 1024;

#[derive(Debug)]
pub struct AssetCommandOutcome {
    pub response: AssetCommandResponse,
    pub emitted_events: Vec<AssetDomainEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedArtifactSource {
    ExtractionSnapshot,
    ReviewReport,
    PagePreview,
    ArchiveManifest,
    ArchivePackage,
    NormalizedTemplate,
}

impl GeneratedArtifactSource {
    fn as_db_str(self) -> &'static str {
        match self {
            Self::ExtractionSnapshot => ASSET_ORIGIN_GENERATED_EXTRACTION_SNAPSHOT,
            Self::ReviewReport => ASSET_ORIGIN_GENERATED_REVIEW_REPORT,
            Self::PagePreview => ASSET_ORIGIN_GENERATED_PAGE_PREVIEW,
            Self::ArchiveManifest => ASSET_ORIGIN_GENERATED_ARCHIVE_MANIFEST,
            Self::ArchivePackage => ASSET_ORIGIN_GENERATED_ARCHIVE_PACKAGE,
            Self::NormalizedTemplate => ASSET_ORIGIN_NORMALIZED_TEMPLATE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssetSourceKind {
    User,
    BusinessDocument,
    GeneratedExtractionSnapshot,
    GeneratedReviewReport,
    GeneratedPagePreview,
    ArchiveManifest,
    ArchivePackage,
    NormalizedTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssetSourceRecord {
    pub asset_id: String,
    pub source: AssetSourceKind,
    pub source_ref: Option<String>,
    pub created_at: i64,
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS assets (
                id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT,
                original_name TEXT NOT NULL,
                kind TEXT NOT NULL CHECK (kind IN ('image', 'video', 'audio', 'document', 'other')),
                mime_type TEXT NOT NULL,
                size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
                sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
                status TEXT NOT NULL CHECK (status IN ('ready', 'failed')),
                revision INTEGER NOT NULL CHECK (revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                preview_available INTEGER NOT NULL DEFAULT 0 CHECK (preview_available IN (0, 1)),
                storage_rel_path TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_assets_project_created
                ON assets(project_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_assets_project_hash
                ON assets(project_id, sha256, status);
            CREATE INDEX IF NOT EXISTS idx_assets_status_updated
                ON assets(status, updated_at DESC);
            "#,
        )
        .map_err(sql_error)?;
    migrate_asset_origins(connection)?;
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS asset_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL CHECK (event_type = 'asset.imported'),
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL,
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                FOREIGN KEY(aggregate_id) REFERENCES assets(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_asset_events_aggregate
                ON asset_events(aggregate_id, sequence);
            CREATE TABLE IF NOT EXISTS asset_command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL CHECK (command_type = 'asset.import'),
                protocol_version TEXT NOT NULL,
                deadline_at INTEGER,
                request_fingerprint TEXT NOT NULL CHECK (length(request_fingerprint) = 64),
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_asset_command_receipts_completed
                ON asset_command_receipts(completed_at);
            "#,
        )
        .map_err(sql_error)
}

fn migrate_asset_origins(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS asset_origins (
                asset_id TEXT PRIMARY KEY NOT NULL,
                origin TEXT NOT NULL CHECK(origin IN (
                    'user',
                    'businessDocument',
                    'generatedExtractionSnapshot',
                    'generatedReviewReport',
                    'generatedPagePreview',
                    'generatedArchiveManifest',
                    'generatedArchivePackage',
                    'normalizedTemplate'
                )),
                origin_ref TEXT,
                created_at INTEGER NOT NULL,
                CHECK(
                    (origin = 'user' AND origin_ref IS NULL)
                    OR (
                        origin IN (
                            'businessDocument',
                            'generatedExtractionSnapshot',
                            'generatedReviewReport',
                            'generatedPagePreview',
                            'generatedArchiveManifest',
                            'generatedArchivePackage',
                            'normalizedTemplate'
                        )
                        AND origin_ref IS NOT NULL
                    )
                ),
                FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_asset_origins_business_ref
                ON asset_origins(origin, origin_ref) WHERE origin_ref IS NOT NULL;
            INSERT OR IGNORE INTO asset_origins (asset_id, origin, origin_ref, created_at)
                SELECT id, 'user', NULL, created_at FROM assets;
            "#,
        )
        .map_err(sql_error)?;

    let schema: String = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'asset_origins'",
            [],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    let supports_generated_artifacts = [
        ASSET_ORIGIN_GENERATED_EXTRACTION_SNAPSHOT,
        ASSET_ORIGIN_GENERATED_REVIEW_REPORT,
        ASSET_ORIGIN_GENERATED_PAGE_PREVIEW,
        ASSET_ORIGIN_GENERATED_ARCHIVE_MANIFEST,
        ASSET_ORIGIN_GENERATED_ARCHIVE_PACKAGE,
        ASSET_ORIGIN_NORMALIZED_TEMPLATE,
    ]
    .iter()
    .all(|origin| schema.contains(origin));
    if supports_generated_artifacts {
        return Ok(());
    }

    let migration = connection.execute_batch(
        r#"
        SAVEPOINT migrate_asset_origins_generated_artifacts;
        DROP INDEX IF EXISTS idx_asset_origins_business_ref;
        ALTER TABLE asset_origins RENAME TO asset_origins_legacy;
        CREATE TABLE asset_origins (
            asset_id TEXT PRIMARY KEY NOT NULL,
            origin TEXT NOT NULL CHECK(origin IN (
                'user',
                'businessDocument',
                'generatedExtractionSnapshot',
                'generatedReviewReport',
                'generatedPagePreview',
                'generatedArchiveManifest',
                'generatedArchivePackage',
                'normalizedTemplate'
            )),
            origin_ref TEXT,
            created_at INTEGER NOT NULL,
            CHECK(
                (origin = 'user' AND origin_ref IS NULL)
                OR (
                    origin IN (
                        'businessDocument',
                        'generatedExtractionSnapshot',
                        'generatedReviewReport',
                        'generatedPagePreview',
                        'generatedArchiveManifest',
                        'generatedArchivePackage',
                        'normalizedTemplate'
                    )
                    AND origin_ref IS NOT NULL
                )
            ),
            FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
        );
        INSERT INTO asset_origins (asset_id, origin, origin_ref, created_at)
            SELECT asset_id, origin, origin_ref, created_at FROM asset_origins_legacy;
        DROP TABLE asset_origins_legacy;
        CREATE UNIQUE INDEX idx_asset_origins_business_ref
            ON asset_origins(origin, origin_ref) WHERE origin_ref IS NOT NULL;
        RELEASE SAVEPOINT migrate_asset_origins_generated_artifacts;
        "#,
    );
    if let Err(error) = migration {
        let _ = connection.execute_batch(
            r#"
            ROLLBACK TO SAVEPOINT migrate_asset_origins_generated_artifacts;
            RELEASE SAVEPOINT migrate_asset_origins_generated_artifacts;
            "#,
        );
        return Err(sql_error(error));
    }
    Ok(())
}

/// Resolves imports whose physical Vault write crossed an ambiguous SQLite
/// COMMIT boundary. Intents are backend-only and never become domain state.
pub(crate) fn reconcile_pending_imports(
    connection: &Connection,
    vault_root: &Path,
) -> Result<(), HostError> {
    let vault_root = prepare_vault_root(vault_root)?;
    let recovery_root = prepare_import_recovery_root(&vault_root)?;
    let reconcile_lock =
        open_recovery_lock_file(&recovery_root.join(IMPORT_RECOVERY_LOCK_FILE), true)?
            .ok_or_else(|| HostError::internal("asset recovery lock was not created"))?;
    reconcile_lock
        .lock_exclusive()
        .map_err(|error| vault_io_error("lock asset import reconciliation", error))?;

    let reconciliation = reconcile_pending_imports_locked(connection, &vault_root, &recovery_root);
    let unlock = FileExt::unlock(&reconcile_lock)
        .map_err(|error| vault_io_error("unlock asset import reconciliation", error));
    reconciliation.and(unlock)
}

fn reconcile_pending_imports_locked(
    connection: &Connection,
    vault_root: &Path,
    recovery_root: &Path,
) -> Result<(), HostError> {
    let entries = fs::read_dir(recovery_root)
        .map_err(|error| vault_io_error("read asset import recovery directory", error))?;
    let mut first_error = None;
    for entry in entries {
        let result = match entry {
            Ok(entry) => {
                reconcile_pending_import_entry(connection, vault_root, recovery_root, entry)
            }
            Err(error) => Err(vault_io_error("read asset recovery entry", error)),
        };
        if first_error.is_none() {
            first_error = result.err();
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn reconcile_pending_import_entry(
    connection: &Connection,
    vault_root: &Path,
    recovery_root: &Path,
    entry: fs::DirEntry,
) -> Result<(), HostError> {
    let file_name = entry.file_name().to_string_lossy().to_string();
    if file_name == IMPORT_RECOVERY_LOCK_FILE || !file_name.ends_with(IMPORT_RECOVERY_INTENT_SUFFIX)
    {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(entry.path())
        .map_err(|error| vault_io_error("inspect asset recovery entry", error))?;
    if is_link_or_reparse(&metadata) {
        return Err(vault_path_error(
            "asset recovery intent cannot be a symlink or reparse point",
        ));
    }
    if !metadata.is_file() {
        return Err(vault_path_error(
            "asset recovery intent must be a regular file",
        ));
    }

    let intent_path = validate_recovery_marker_path(recovery_root, &entry.path())?;
    let mut intent_file = open_recovery_lock_file(&intent_path, false)?
        .ok_or_else(|| vault_path_error("asset recovery intent disappeared"))?;
    match intent_file.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if is_lock_contended(&error) => return Ok(()),
        Err(error) => return Err(vault_io_error("inspect active asset import intent", error)),
    }

    let settlement = settle_recovery_intent(connection, vault_root, &intent_path, &mut intent_file);
    let unlock = FileExt::unlock(&intent_file)
        .map_err(|error| vault_io_error("unlock asset recovery intent", error));
    drop(intent_file);
    let clear_intent = settlement?;
    unlock?;
    if clear_intent {
        remove_recovery_marker_if_safe(recovery_root, &intent_path)?;
        let _ = sync_directory(recovery_root);
    }
    Ok(())
}

fn settle_recovery_intent(
    connection: &Connection,
    vault_root: &Path,
    intent_path: &Path,
    intent_file: &mut File,
) -> Result<bool, HostError> {
    let metadata = intent_file
        .metadata()
        .map_err(|error| vault_io_error("inspect opened asset recovery intent", error))?;
    if metadata.len() > MAX_IMPORT_RECOVERY_INTENT_BYTES {
        return Err(asset_recovery_error(
            "asset recovery intent exceeds the size limit",
        ));
    }
    let mut encoded = Vec::with_capacity(metadata.len() as usize);
    intent_file
        .take(MAX_IMPORT_RECOVERY_INTENT_BYTES + 1)
        .read_to_end(&mut encoded)
        .map_err(|error| vault_io_error("read asset recovery intent", error))?;
    if encoded.len() as u64 > MAX_IMPORT_RECOVERY_INTENT_BYTES {
        return Err(asset_recovery_error(
            "asset recovery intent exceeds the size limit",
        ));
    }
    let intent: PendingImportIntent = serde_json::from_slice(&encoded)
        .map_err(|error| asset_recovery_error(format!("invalid asset recovery JSON: {error}")))?;
    validate_pending_import_intent(&intent, intent_path)?;

    let relative = storage_relative_path(&intent.sha256, &intent.asset_id, &intent.extension);
    let final_path = join_relative_path(vault_root, &relative)?;
    match find_asset_storage(connection, &intent.asset_id)? {
        Some(stored) => {
            if stored.storage_rel_path != relative
                || stored.sha256 != intent.sha256
                || stored.size_bytes != intent.size_bytes
            {
                return Err(asset_recovery_error(
                    "committed asset storage does not match its recovery intent",
                ));
            }
            validate_recovery_candidate(vault_root, &final_path, intent.size_bytes)?;
            Ok(true)
        }
        None => {
            let references = summarize_storage_references(
                connection,
                &relative,
                &intent.sha256,
                intent.size_bytes,
            )?;
            if references.total == 0 {
                remove_recovery_candidate(vault_root, &final_path, intent.size_bytes)?;
            } else if references.total == references.matching {
                // Another logical Asset can legitimately reuse this physical
                // file. A stale marker for the original owner must never delete
                // storage that is still authoritative for a deduplicated sibling.
                validate_recovery_candidate(vault_root, &final_path, intent.size_bytes)?;
            } else {
                return Err(asset_recovery_error(
                    "recovered Vault asset has conflicting database references",
                ));
            }
            Ok(true)
        }
    }
}

/// Imports one logical asset. Every call creates a distinct AssetRecord, while
/// records in the same project may reference the first durable copy of an
/// identical hash. Cross-project files are deliberately not deduplicated so
/// ownership, deletion, export and future sync policies remain isolated.
pub fn import_file(
    connection: &mut Connection,
    vault_root: &Path,
    project_id: Option<&str>,
    source_path: &Path,
) -> Result<AssetRecord, HostError> {
    import_file_with_origin(
        connection,
        vault_root,
        project_id,
        source_path,
        AssetOrigin::User,
    )
}

pub(crate) fn import_business_document(
    connection: &mut Connection,
    vault_root: &Path,
    project_id: &str,
    source_path: &Path,
    generation_id: &str,
) -> Result<AssetRecord, HostError> {
    let generation_id = Uuid::parse_str(generation_id)
        .map(|value| value.to_string())
        .map_err(|_| HostError::validation("generationId must be a UUID"))?;
    import_file_with_origin(
        connection,
        vault_root,
        Some(project_id),
        source_path,
        AssetOrigin::BusinessDocument(generation_id),
    )
}

/// Imports an immutable backend-generated artifact into the Local Vault. The
/// `(source, source_ref)` pair is its idempotency identity: retries return the
/// already committed AssetRecord without touching the staging file again.
pub(crate) fn import_generated_artifact(
    connection: &mut Connection,
    vault_root: &Path,
    project_id: &str,
    source_path: &Path,
    source: GeneratedArtifactSource,
    source_ref: &str,
) -> Result<AssetRecord, HostError> {
    let project_id = normalize_project_id(Some(project_id))?
        .expect("a generated artifact always requires a project id");
    let source_ref = normalize_generated_artifact_ref(source_ref)?;
    if let Some(existing) = find_asset_by_origin_ref(connection, source.as_db_str(), &source_ref)? {
        return ensure_generated_artifact_project(existing, &project_id);
    }

    let result = import_file_with_origin(
        connection,
        vault_root,
        Some(&project_id),
        source_path,
        AssetOrigin::GeneratedArtifact {
            source,
            source_ref: source_ref.clone(),
        },
    );
    match result {
        Ok(asset) => Ok(asset),
        Err(error) => {
            // A concurrent importer may have committed the same immutable
            // source identity after the preflight lookup. Prefer that stable
            // asset over manufacturing a second logical record.
            match find_asset_by_origin_ref(connection, source.as_db_str(), &source_ref)? {
                Some(existing) => ensure_generated_artifact_project(existing, &project_id),
                None => Err(error),
            }
        }
    }
}

enum AssetOrigin {
    User,
    BusinessDocument(String),
    GeneratedArtifact {
        source: GeneratedArtifactSource,
        source_ref: String,
    },
}

fn import_file_with_origin(
    connection: &mut Connection,
    vault_root: &Path,
    project_id: Option<&str>,
    source_path: &Path,
    origin: AssetOrigin,
) -> Result<AssetRecord, HostError> {
    let mut prepared = prepare_import(vault_root, project_id, source_path)?;
    validate_prepared_asset_origin(&prepared.asset, &origin)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let (asset, mut pending_file) = persist_prepared_asset(&transaction, &mut prepared, &origin)?;

    match transaction.commit() {
        Ok(()) => {
            mark_optional_committed(&mut pending_file);
            Ok(asset)
        }
        Err(error) => {
            // COMMIT failures are ambiguous. A visible row proves this exact
            // generated asset was committed. If the verification query also
            // fails, retain the durable recovery intent instead of leaking an
            // untracked file or deleting a possibly authoritative one.
            match find_asset(connection, &asset.id) {
                Ok(Some(persisted)) => {
                    mark_optional_committed(&mut pending_file);
                    Ok(persisted)
                }
                Ok(None) => Err(sql_error(error)),
                Err(_) => {
                    preserve_optional_for_recovery(&mut pending_file);
                    Err(sql_error(error))
                }
            }
        }
    }
}

pub fn execute_import_command(
    connection: &mut Connection,
    vault_root: &Path,
    command: AssetCommandEnvelope,
    resolved_source_path: &Path,
) -> Result<AssetCommandOutcome, HostError> {
    execute_import_command_with_resolver(connection, vault_root, command, || {
        Ok(resolved_source_path.to_path_buf())
    })
}

/// Keeps one-shot source-token consumption behind the durable receipt check.
/// Host adapters must resolve/consume their token inside this closure.
pub fn execute_import_command_with_resolver<F>(
    connection: &mut Connection,
    vault_root: &Path,
    command: AssetCommandEnvelope,
    resolver: F,
) -> Result<AssetCommandOutcome, HostError>
where
    F: FnOnce() -> Result<PathBuf, HostError>,
{
    validate_asset_command(&command)?;
    let meta = asset_command_meta(&command);
    let fingerprint = asset_command_fingerprint(&command)?;

    // Receipt lookup deliberately happens before source access and deadline
    // validation. A committed command remains replayable after its source token
    // expires, its selected file disappears, or its original deadline elapses.
    {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        if let Some(response) = find_existing_asset_receipt(
            &transaction,
            &meta.command_id,
            &meta.idempotency_key,
            &fingerprint,
        )? {
            transaction.commit().map_err(sql_error)?;
            return Ok(AssetCommandOutcome {
                response,
                emitted_events: Vec::new(),
            });
        }
        validate_deadline(meta.deadline_at)?;
        transaction.commit().map_err(sql_error)?;
    }

    let resolved_source_path = resolver()?;
    let mut prepared = prepare_import(
        vault_root,
        meta.project_id.as_deref(),
        &resolved_source_path,
    )?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;

    // The source copy can be long-running. Recheck under the write lock to
    // close the race with another process that committed the same command.
    if let Some(response) = find_existing_asset_receipt(
        &transaction,
        &meta.command_id,
        &meta.idempotency_key,
        &fingerprint,
    )? {
        transaction.commit().map_err(sql_error)?;
        return Ok(AssetCommandOutcome {
            response,
            emitted_events: Vec::new(),
        });
    }
    validate_deadline(meta.deadline_at)?;

    let (asset, mut pending_file) =
        persist_prepared_asset(&transaction, &mut prepared, &AssetOrigin::User)?;
    let event = append_asset_event(&transaction, &asset, &meta.trace_id)?;
    let completed_at = now_millis();
    let response = AssetCommandResponse {
        receipt: CommandReceipt {
            command_id: meta.command_id.clone(),
            idempotency_key: meta.idempotency_key.clone(),
            command_type: "asset.import".to_string(),
            aggregate_id: asset.id.clone(),
            revision: asset.revision,
            last_event_sequence: event.sequence,
            completed_at,
        },
        asset,
        replayed: false,
    };
    let response_json = serde_json::to_string(&response).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO asset_command_receipts
             (idempotency_key, command_id, command_type, protocol_version, deadline_at,
              request_fingerprint, response_json, completed_at)
             VALUES (?1, ?2, 'asset.import', ?3, ?4, ?5, ?6, ?7)",
            params![
                meta.idempotency_key,
                meta.command_id,
                meta.protocol_version,
                meta.deadline_at,
                fingerprint,
                response_json,
                completed_at,
            ],
        )
        .map_err(sql_error)?;

    match transaction.commit() {
        Ok(()) => {
            mark_optional_committed(&mut pending_file);
            Ok(AssetCommandOutcome {
                response,
                emitted_events: vec![event],
            })
        }
        Err(error) => match find_existing_asset_receipt(
            connection,
            &meta.command_id,
            &meta.idempotency_key,
            &fingerprint,
        ) {
            Ok(Some(persisted)) => {
                settle_ambiguous_import_commit(connection, &mut pending_file, &response, persisted)
            }
            Ok(None) => Err(sql_error(error)),
            Err(_) => {
                // The durable intent lets startup reconciliation decide after
                // SQLite becomes readable again.
                preserve_optional_for_recovery(&mut pending_file);
                Err(sql_error(error))
            }
        },
    }
}

pub fn replay_asset_events(
    connection: &Connection,
    after_sequence: i64,
    limit: u32,
) -> Result<Vec<AssetDomainEvent>, HostError> {
    if after_sequence < 0 {
        return Err(HostError::validation("afterSequence cannot be negative"));
    }
    let limit = limit.clamp(1, 1_000) as i64;
    let mut statement = connection
        .prepare(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, payload_json
             FROM asset_events WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map(params![after_sequence, limit], asset_event_from_row)
        .map_err(sql_error)?;
    let events = rows.collect::<Result<Vec<_>, _>>().map_err(sql_error)?;
    Ok(events)
}

fn settle_ambiguous_import_commit(
    connection: &Connection,
    pending_file: &mut Option<PendingAssetFile>,
    attempted: &AssetCommandResponse,
    mut persisted: AssetCommandResponse,
) -> Result<AssetCommandOutcome, HostError> {
    // A concurrent winner can own the same command identity while referencing
    // a different generated asset. Only an exact asset ID match proves this
    // invocation's physical file and event committed.
    let committed_by_this_invocation = attempted.asset.id == persisted.asset.id;
    settle_optional_for_persisted_asset(pending_file, &attempted.asset.id, &persisted.asset.id);
    if committed_by_this_invocation {
        persisted.replayed = false;
        let persisted_event = load_asset_event(connection, persisted.receipt.last_event_sequence)?;
        Ok(AssetCommandOutcome {
            response: persisted,
            emitted_events: vec![persisted_event],
        })
    } else {
        persisted.replayed = true;
        Ok(AssetCommandOutcome {
            response: persisted,
            emitted_events: Vec::new(),
        })
    }
}
pub fn list_assets(
    connection: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<AssetRecord>, HostError> {
    let mut assets = Vec::new();
    if let Some(project_id) = project_id {
        let project_id = normalize_project_id(Some(project_id))?.expect("normalized project id");
        let mut statement = connection
            .prepare(
                "SELECT id, project_id, original_name, kind, mime_type, size_bytes, sha256,
                        status, revision, created_at, updated_at, preview_available
                 FROM assets WHERE project_id = ?1
                 ORDER BY created_at DESC, id DESC",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map([project_id], asset_from_row)
            .map_err(sql_error)?;
        for row in rows {
            assets.push(row.map_err(sql_error)?);
        }
    } else {
        let mut statement = connection
            .prepare(
                "SELECT id, project_id, original_name, kind, mime_type, size_bytes, sha256,
                        status, revision, created_at, updated_at, preview_available
                 FROM assets ORDER BY created_at DESC, id DESC",
            )
            .map_err(sql_error)?;
        let rows = statement.query_map([], asset_from_row).map_err(sql_error)?;
        for row in rows {
            assets.push(row.map_err(sql_error)?);
        }
    }
    Ok(assets)
}

pub fn get_asset(connection: &Connection, asset_id: &str) -> Result<AssetRecord, HostError> {
    normalize_asset_id(asset_id)?;
    find_asset(connection, asset_id)?
        .ok_or_else(|| HostError::new("ASSET_NOT_FOUND", "asset does not exist", false))
}

pub(crate) fn get_asset_source(
    connection: &Connection,
    asset_id: &str,
) -> Result<AssetSourceRecord, HostError> {
    normalize_asset_id(asset_id)?;
    connection
        .query_row(
            "SELECT origin.asset_id, origin.origin, origin.origin_ref, origin.created_at
             FROM asset_origins origin
             JOIN assets asset ON asset.id = origin.asset_id
             WHERE origin.asset_id = ?1",
            [asset_id],
            asset_source_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("ASSET_NOT_FOUND", "asset does not exist", false))
}

/// Resolves an internal native path for backend media operations. This function
/// must never be exposed through a serialized command or event response.
pub(crate) fn resolve_original_path(
    connection: &Connection,
    vault_root: &Path,
    asset_id: &str,
) -> Result<PathBuf, HostError> {
    normalize_asset_id(asset_id)?;
    let relative: Option<String> = connection
        .query_row(
            "SELECT storage_rel_path FROM assets WHERE id = ?1 AND status = 'ready'",
            [asset_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    let relative = relative
        .ok_or_else(|| HostError::new("ASSET_NOT_FOUND", "ready asset does not exist", false))?;
    let vault_root = prepare_vault_root(vault_root)?;
    resolve_existing_storage(&vault_root, &relative)
}

/// Resolves the service-owned storage location even when the final file has
/// already disappeared during an interrupted cleanup. The returned path is
/// still confined to a canonical Vault parent and is never serialized.
/// Resolves and verifies a ready Vault asset before native open/export operations.
/// The native path never crosses the serialized Host boundary.
pub(crate) fn verify_ready_asset_integrity(
    connection: &Connection,
    vault_root: &Path,
    asset_id: &str,
) -> Result<(AssetRecord, PathBuf), HostError> {
    let asset = get_asset(connection, asset_id)?;
    if asset.status != AssetStatus::Ready {
        return Err(HostError::new(
            "ASSET_NOT_READY",
            "asset is not ready for native access",
            true,
        ));
    }
    let path = resolve_original_path(connection, vault_root, asset_id)?;
    let metadata = fs::metadata(&path)
        .map_err(|error| vault_io_error("inspect Vault asset before native access", error))?;
    if metadata.len() != asset.size_bytes as u64 {
        return Err(HostError::new(
            "VAULT_ASSET_INTEGRITY_MISMATCH",
            "Vault asset size no longer matches the authoritative record",
            false,
        ));
    }

    let mut file = File::open(&path)
        .map_err(|error| vault_io_error("open Vault asset for integrity verification", error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut observed_size = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            vault_io_error("read Vault asset for integrity verification", error)
        })?;
        if read == 0 {
            break;
        }
        observed_size = observed_size.checked_add(read as u64).ok_or_else(|| {
            HostError::new(
                "VAULT_ASSET_INTEGRITY_MISMATCH",
                "Vault asset size overflowed during integrity verification",
                false,
            )
        })?;
        hasher.update(&buffer[..read]);
    }
    let observed_sha256 = format!("{:x}", hasher.finalize());
    if observed_size != asset.size_bytes as u64 || observed_sha256 != asset.sha256 {
        return Err(HostError::new(
            "VAULT_ASSET_INTEGRITY_MISMATCH",
            "Vault asset content no longer matches the authoritative record",
            false,
        ));
    }
    Ok((asset, path))
}

/// Reads a ready template from the Local Vault into a bounded in-memory buffer.
/// Integrity verification and byte collection use the same open file handle.
pub(crate) fn read_verified_template_asset(
    connection: &Connection,
    vault_root: &Path,
    asset_id: &str,
) -> Result<(AssetRecord, Vec<u8>), HostError> {
    let asset = get_asset(connection, asset_id)?;
    if asset.status != AssetStatus::Ready {
        return Err(HostError::new(
            "ASSET_NOT_READY",
            "asset is not ready for template rendering",
            true,
        ));
    }
    if asset.size_bytes < 0 || asset.size_bytes as u64 > MAX_TEMPLATE_ASSET_SIZE_BYTES {
        return Err(HostError::new(
            "TEMPLATE_ASSET_TOO_LARGE",
            format!(
                "template asset exceeds the {} byte safety limit",
                MAX_TEMPLATE_ASSET_SIZE_BYTES
            ),
            false,
        ));
    }

    let path = resolve_original_path(connection, vault_root, asset_id)?;
    let mut file =
        File::open(&path).map_err(|error| vault_io_error("open Vault template asset", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| vault_io_error("inspect Vault template asset", error))?;
    if metadata.len() != asset.size_bytes as u64 {
        return Err(HostError::new(
            "VAULT_ASSET_INTEGRITY_MISMATCH",
            "Vault template asset size no longer matches the authoritative record",
            false,
        ));
    }

    let expected_size = usize::try_from(asset.size_bytes).map_err(|_| {
        HostError::new(
            "TEMPLATE_ASSET_TOO_LARGE",
            "template asset size cannot be represented in memory",
            false,
        )
    })?;
    let mut bytes = Vec::with_capacity(expected_size);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| vault_io_error("read Vault template asset", error))?;
        if read == 0 {
            break;
        }
        let observed_size = bytes.len().checked_add(read).ok_or_else(|| {
            HostError::new(
                "TEMPLATE_ASSET_TOO_LARGE",
                "template asset size overflowed during bounded read",
                false,
            )
        })?;
        if observed_size > expected_size || observed_size as u64 > MAX_TEMPLATE_ASSET_SIZE_BYTES {
            return Err(HostError::new(
                "VAULT_ASSET_INTEGRITY_MISMATCH",
                "Vault template asset grew beyond the authoritative size during read",
                false,
            ));
        }
        hasher.update(&buffer[..read]);
        bytes.extend_from_slice(&buffer[..read]);
    }

    let observed_sha256 = format!("{:x}", hasher.finalize());
    if bytes.len() != expected_size || observed_sha256 != asset.sha256 {
        return Err(HostError::new(
            "VAULT_ASSET_INTEGRITY_MISMATCH",
            "Vault template asset content no longer matches the authoritative record",
            false,
        ));
    }
    Ok((asset, bytes))
}

/// Reads a ready Vault asset into memory under a caller-provided safety limit.
/// Integrity verification and byte collection use the same open file handle.
pub(crate) fn read_verified_asset_limited(
    connection: &Connection,
    vault_root: &Path,
    asset_id: &str,
    max_size_bytes: u64,
) -> Result<(AssetRecord, Vec<u8>), HostError> {
    if max_size_bytes == 0 || max_size_bytes > MAX_ASSET_SIZE_BYTES {
        return Err(HostError::new(
            "ASSET_READ_LIMIT_INVALID",
            "asset read limit must be within the supported Vault asset range",
            false,
        ));
    }
    let asset = get_asset(connection, asset_id)?;
    if asset.status != AssetStatus::Ready {
        return Err(HostError::new(
            "ASSET_NOT_READY",
            "asset is not ready for rendering",
            true,
        ));
    }
    if asset.size_bytes < 0 || asset.size_bytes as u64 > max_size_bytes {
        return Err(HostError::new(
            "ASSET_TOO_LARGE",
            format!("asset exceeds the {max_size_bytes} byte safety limit"),
            false,
        ));
    }

    let path = resolve_original_path(connection, vault_root, asset_id)?;
    let mut file = File::open(&path)
        .map_err(|error| vault_io_error("open Vault asset for bounded read", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| vault_io_error("inspect Vault asset for bounded read", error))?;
    if metadata.len() != asset.size_bytes as u64 {
        return Err(HostError::new(
            "VAULT_ASSET_INTEGRITY_MISMATCH",
            "Vault asset size no longer matches the authoritative record",
            false,
        ));
    }

    let expected_size = usize::try_from(asset.size_bytes).map_err(|_| {
        HostError::new(
            "ASSET_TOO_LARGE",
            "asset size cannot be represented in memory",
            false,
        )
    })?;
    let mut bytes = Vec::with_capacity(expected_size);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| vault_io_error("read Vault asset into bounded buffer", error))?;
        if read == 0 {
            break;
        }
        let observed_size = bytes.len().checked_add(read).ok_or_else(|| {
            HostError::new(
                "ASSET_TOO_LARGE",
                "asset size overflowed during bounded read",
                false,
            )
        })?;
        if observed_size > expected_size || observed_size as u64 > max_size_bytes {
            return Err(HostError::new(
                "VAULT_ASSET_INTEGRITY_MISMATCH",
                "Vault asset grew beyond the authoritative size during read",
                false,
            ));
        }
        hasher.update(&buffer[..read]);
        bytes.extend_from_slice(&buffer[..read]);
    }

    let observed_sha256 = format!("{:x}", hasher.finalize());
    if bytes.len() != expected_size || observed_sha256 != asset.sha256 {
        return Err(HostError::new(
            "VAULT_ASSET_INTEGRITY_MISMATCH",
            "Vault asset content no longer matches the authoritative record",
            false,
        ));
    }
    Ok((asset, bytes))
}

/// Copies a verified asset to a user-selected path without exposing Vault paths to the UI.
pub(crate) fn export_verified_asset_to_path(
    connection: &Connection,
    vault_root: &Path,
    asset_id: &str,
    destination: &Path,
) -> Result<(), HostError> {
    let (asset, source) = verify_ready_asset_integrity(connection, vault_root, asset_id)?;
    let file_name = destination.file_name().ok_or_else(|| {
        HostError::validation("asset export destination must include a file name")
    })?;
    if file_name.is_empty() {
        return Err(HostError::validation(
            "asset export destination must include a file name",
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        HostError::validation("asset export destination must include a parent directory")
    })?;
    let resolved_parent = fs::canonicalize(parent)
        .map_err(|error| source_io_error("resolve asset export directory", error))?;
    let resolved_vault = prepare_vault_root(vault_root)?;
    if resolved_parent.starts_with(&resolved_vault) {
        return Err(HostError::new(
            "ASSET_EXPORT_TARGET_INVALID",
            "asset export destination cannot be inside the Local Vault",
            false,
        ));
    }
    if destination.exists() {
        let destination_metadata = fs::symlink_metadata(destination)
            .map_err(|error| source_io_error("inspect asset export destination", error))?;
        if destination_metadata.file_type().is_symlink() || !destination_metadata.is_file() {
            return Err(HostError::new(
                "ASSET_EXPORT_TARGET_INVALID",
                "asset export destination must be a regular file",
                false,
            ));
        }
        let resolved_destination = fs::canonicalize(destination)
            .map_err(|error| source_io_error("resolve asset export destination", error))?;
        if resolved_destination.starts_with(&resolved_vault) {
            return Err(HostError::new(
                "ASSET_EXPORT_TARGET_INVALID",
                "asset export destination cannot replace a Local Vault object",
                false,
            ));
        }
    }

    fs::copy(&source, destination)
        .map_err(|error| source_io_error("copy asset to export destination", error))?;
    let mut exported = File::open(destination)
        .map_err(|error| source_io_error("open exported asset for verification", error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut exported_size = 0_u64;
    loop {
        let read = exported
            .read(&mut buffer)
            .map_err(|error| source_io_error("read exported asset for verification", error))?;
        if read == 0 {
            break;
        }
        exported_size = exported_size.checked_add(read as u64).ok_or_else(|| {
            HostError::new(
                "ASSET_EXPORT_INTEGRITY_MISMATCH",
                "exported asset size overflowed during verification",
                false,
            )
        })?;
        hasher.update(&buffer[..read]);
    }
    let exported_sha256 = format!("{:x}", hasher.finalize());
    if exported_size != asset.size_bytes as u64 || exported_sha256 != asset.sha256 {
        let _ = fs::remove_file(destination);
        return Err(HostError::new(
            "ASSET_EXPORT_INTEGRITY_MISMATCH",
            "exported asset did not match the authoritative Local Vault object",
            true,
        ));
    }
    Ok(())
}

pub(crate) fn resolve_storage_path_for_cleanup(
    connection: &Connection,
    vault_root: &Path,
    asset_id: &str,
) -> Result<PathBuf, HostError> {
    normalize_asset_id(asset_id)?;
    let relative: String = connection
        .query_row(
            "SELECT storage_rel_path FROM assets WHERE id = ?1",
            [asset_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("ASSET_NOT_FOUND", "asset does not exist", false))?;
    let vault_root = prepare_vault_root(vault_root)?;
    let candidate = join_relative_path(&vault_root, &relative)?;
    let parent = candidate
        .parent()
        .ok_or_else(|| HostError::internal("Vault asset path does not have a parent"))?;
    let resolved_parent = fs::canonicalize(parent)
        .map_err(|error| vault_io_error("resolve Vault asset parent", error))?;
    if !resolved_parent.starts_with(&vault_root) {
        return Err(HostError::new(
            "VAULT_PATH_INVALID",
            "stored Vault asset parent escapes the Vault root",
            false,
        ));
    }
    match fs::symlink_metadata(&candidate) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(HostError::new(
            "VAULT_PATH_INVALID",
            "stored Vault asset cannot be a symbolic link",
            false,
        )),
        Ok(_) => {
            let resolved = fs::canonicalize(&candidate)
                .map_err(|error| vault_io_error("resolve Vault cleanup path", error))?;
            if !resolved.starts_with(&vault_root) {
                return Err(HostError::new(
                    "VAULT_PATH_INVALID",
                    "stored Vault cleanup path escapes the Vault root",
                    false,
                ));
            }
            Ok(resolved)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(candidate),
        Err(error) => Err(vault_io_error("inspect Vault cleanup path", error)),
    }
}

#[derive(Debug)]
struct AssetCommandMeta {
    command_id: String,
    protocol_version: String,
    trace_id: String,
    idempotency_key: String,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
    project_id: Option<String>,
}

fn asset_command_meta(command: &AssetCommandEnvelope) -> AssetCommandMeta {
    match command {
        AssetCommandEnvelope::Import {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => AssetCommandMeta {
            command_id: command_id.clone(),
            protocol_version: protocol_version.clone(),
            trace_id: context.trace_id.clone(),
            idempotency_key: idempotency_key.clone(),
            expected_revision: *expected_revision,
            deadline_at: *deadline_at,
            project_id: payload.project_id.clone(),
        },
    }
}

fn validate_asset_command(command: &AssetCommandEnvelope) -> Result<(), HostError> {
    let meta = asset_command_meta(command);
    let (context, source_token) = match command {
        AssetCommandEnvelope::Import {
            context, payload, ..
        } => (context, &payload.source_token),
    };
    if !is_legacy_surface_protocol_supported(&meta.protocol_version) {
        return Err(HostError::new(
            "PROTOCOL_VERSION_UNSUPPORTED",
            format!(
                "expected protocolVersion {LEGACY_PROTOCOL_VERSION}, {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, or {PROTOCOL_VERSION}, received {}",
                meta.protocol_version,
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
    if context.actor_id.trim().is_empty()
        || context.trace_id.trim().is_empty()
        || context.window_id.trim().is_empty()
    {
        return Err(HostError::validation(
            "actorId, traceId and windowId are required",
        ));
    }
    if meta.expected_revision.is_some() {
        return Err(HostError::validation(
            "asset.import rejects expectedRevision",
        ));
    }
    let token_length = source_token.trim().chars().count();
    if !(1..=2_048).contains(&token_length) {
        return Err(HostError::validation("sourceToken length must be 1..2048"));
    }
    let payload_project_id = normalize_project_id(meta.project_id.as_deref())?;
    let context_project_id = normalize_project_id(context.project_id.as_deref())?;
    if context_project_id.is_some()
        && payload_project_id.is_some()
        && context_project_id != payload_project_id
    {
        return Err(HostError::validation(
            "context projectId must match asset payload projectId",
        ));
    }
    Ok(())
}

fn asset_command_fingerprint(command: &AssetCommandEnvelope) -> Result<String, HostError> {
    let value = match command {
        AssetCommandEnvelope::Import {
            protocol_version,
            context,
            payload,
            expected_revision,
            ..
        } => serde_json::json!({
            "commandType": "asset.import",
            "protocolVersion": protocol_version,
            "actorId": context.actor_id,
            "contextProjectId": context.project_id,
            "expectedRevision": expected_revision,
            "payload": {
                "sourceToken": payload.source_token,
                "projectId": payload.project_id,
            },
        }),
    };
    let bytes = serde_json::to_vec(&value).map_err(json_error)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
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

fn find_existing_asset_receipt(
    connection: &Connection,
    command_id: &str,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<Option<AssetCommandResponse>, HostError> {
    let by_key: Option<(String, String, String)> = connection
        .query_row(
            "SELECT command_id, request_fingerprint, response_json
             FROM asset_command_receipts WHERE idempotency_key = ?1",
            [idempotency_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(sql_error)?;
    if let Some((stored_command_id, stored_fingerprint, response_json)) = by_key {
        if stored_fingerprint != fingerprint {
            return Err(HostError::new(
                "IDEMPOTENCY_KEY_REUSED",
                "idempotencyKey reused for a different asset request",
                false,
            ));
        }
        if stored_command_id != command_id {
            let command_is_already_used = connection
                .query_row(
                    "SELECT 1 FROM asset_command_receipts WHERE command_id = ?1",
                    [command_id],
                    |_| Ok(()),
                )
                .optional()
                .map_err(sql_error)?
                .is_some();
            if command_is_already_used {
                return Err(HostError::new(
                    "COMMAND_ID_REUSED",
                    "commandId is already bound to another asset command",
                    false,
                ));
            }
        }
        let mut response: AssetCommandResponse =
            serde_json::from_str(&response_json).map_err(json_error)?;
        response.replayed = true;
        return Ok(Some(response));
    }

    let by_command: Option<String> = connection
        .query_row(
            "SELECT idempotency_key FROM asset_command_receipts WHERE command_id = ?1",
            [command_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    if by_command.is_some() {
        return Err(HostError::new(
            "COMMAND_ID_REUSED",
            "commandId reused with a different idempotencyKey",
            false,
        ));
    }
    Ok(None)
}

pub(crate) fn append_asset_event(
    transaction: &Transaction<'_>,
    asset: &AssetRecord,
    trace_id: &str,
) -> Result<AssetDomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    let occurred_at = now_millis();
    let payload_json = serde_json::to_string(asset).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO asset_events
             (event_id, event_type, aggregate_id, revision, occurred_at, trace_id, payload_json)
             VALUES (?1, 'asset.imported', ?2, ?3, ?4, ?5, ?6)",
            params![
                event_id,
                asset.id,
                asset.revision,
                occurred_at,
                trace_id,
                payload_json,
            ],
        )
        .map_err(sql_error)?;
    Ok(AssetDomainEvent {
        sequence: transaction.last_insert_rowid(),
        event_id,
        event_type: AssetEventType::Imported,
        aggregate_id: asset.id.clone(),
        revision: asset.revision,
        occurred_at,
        trace_id: trace_id.to_string(),
        asset: asset.clone(),
    })
}

fn load_asset_event(connection: &Connection, sequence: i64) -> Result<AssetDomainEvent, HostError> {
    connection
        .query_row(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, payload_json
             FROM asset_events WHERE sequence = ?1",
            [sequence],
            asset_event_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::internal("committed asset event could not be recovered"))
}

fn asset_event_from_row(row: &Row<'_>) -> rusqlite::Result<AssetDomainEvent> {
    let event_type_value: String = row.get(2)?;
    if event_type_value != AssetEventType::Imported.as_wire_str() {
        return Err(conversion_error(&event_type_value));
    }
    let payload_json: String = row.get(7)?;
    let asset = serde_json::from_str(&payload_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            payload_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(AssetDomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type: AssetEventType::Imported,
        aggregate_id: row.get(3)?,
        revision: row.get(4)?,
        occurred_at: row.get(5)?,
        trace_id: row.get(6)?,
        asset,
    })
}

struct PreparedImport {
    vault_root: PathBuf,
    staging_path: PathBuf,
    staging_cleanup: CleanupFile,
    asset: AssetRecord,
    extension: &'static str,
}

fn prepare_import(
    vault_root: &Path,
    project_id: Option<&str>,
    source_path: &Path,
) -> Result<PreparedImport, HostError> {
    let project_id = normalize_project_id(project_id)?;
    let original_name = original_name(source_path)?;
    let source_metadata = fs::symlink_metadata(source_path)
        .map_err(|error| source_io_error("inspect source file", error))?;
    if source_metadata.file_type().is_symlink() || !source_metadata.file_type().is_file() {
        return Err(HostError::new(
            "ASSET_SOURCE_INVALID",
            "asset source must be a regular file and cannot be a symbolic link",
            false,
        ));
    }
    validate_size(source_metadata.len())?;

    let mut source =
        File::open(source_path).map_err(|error| source_io_error("open source file", error))?;
    let opened_size = source
        .metadata()
        .map_err(|error| source_io_error("inspect opened source file", error))?
        .len();
    validate_size(opened_size)?;

    let vault_root = prepare_vault_root(vault_root)?;
    let staging_root = vault_root.join(".staging");
    prepare_internal_asset_directory(&vault_root, &staging_root)?;
    let asset_id = Uuid::new_v4().to_string();
    let staging_path = staging_root.join(format!("{asset_id}.part"));
    let staging_cleanup = CleanupFile::new(staging_path.clone());
    let mut staging = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging_path)
        .map_err(|error| vault_io_error("create Vault staging file", error))?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut sniffed = Vec::with_capacity(SNIFF_BYTES);
    let mut copied_size = 0_u64;
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| source_io_error("read source file", error))?;
        if read == 0 {
            break;
        }
        copied_size = copied_size
            .checked_add(read as u64)
            .ok_or_else(|| HostError::new("ASSET_TOO_LARGE", "asset size overflow", false))?;
        validate_size(copied_size)?;
        if sniffed.len() < SNIFF_BYTES {
            let remaining = SNIFF_BYTES - sniffed.len();
            sniffed.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        hasher.update(&buffer[..read]);
        staging
            .write_all(&buffer[..read])
            .map_err(|error| vault_io_error("write Vault staging file", error))?;
    }
    if copied_size != opened_size {
        return Err(HostError::new(
            "ASSET_SOURCE_CHANGED",
            "asset source size changed while it was being imported",
            true,
        ));
    }
    staging
        .flush()
        .map_err(|error| vault_io_error("flush Vault staging file", error))?;
    staging
        .sync_all()
        .map_err(|error| vault_io_error("sync Vault staging file", error))?;
    drop(staging);

    let sha256 = format!("{:x}", hasher.finalize());
    let detected = detect_type(source_path, &sniffed);
    let now = now_millis();
    Ok(PreparedImport {
        vault_root,
        staging_path,
        staging_cleanup,
        asset: AssetRecord {
            id: asset_id,
            project_id,
            original_name,
            kind: detected.kind,
            mime_type: detected.mime_type.to_string(),
            size_bytes: copied_size as i64,
            sha256,
            status: AssetStatus::Ready,
            revision: 1,
            created_at: now,
            updated_at: now,
            preview_available: false,
        },
        extension: detected.extension,
    })
}

fn persist_prepared_asset(
    transaction: &Transaction<'_>,
    prepared: &mut PreparedImport,
    origin: &AssetOrigin,
) -> Result<(AssetRecord, Option<PendingAssetFile>), HostError> {
    let reusable_path = find_reusable_storage(
        transaction,
        &prepared.vault_root,
        prepared.asset.project_id.as_deref(),
        &prepared.asset.sha256,
        prepared.asset.size_bytes,
    )?;
    let mut pending_file = None;
    let storage_rel_path = if let Some(relative) = reusable_path {
        relative
    } else {
        let relative = storage_relative_path(
            &prepared.asset.sha256,
            &prepared.asset.id,
            prepared.extension,
        );
        let final_path = join_relative_path(&prepared.vault_root, &relative)?;
        let parent = final_path
            .parent()
            .ok_or_else(|| {
                HostError::internal("Vault asset path does not have a parent directory")
            })?
            .to_path_buf();
        prepare_internal_asset_directory(&prepared.vault_root, &parent)?;
        let pending = create_pending_asset_file(
            &prepared.vault_root,
            final_path.clone(),
            &prepared.asset,
            prepared.extension,
        )?;
        fs::rename(&prepared.staging_path, &final_path)
            .map_err(|error| vault_io_error("commit Vault asset file", error))?;
        prepared.staging_cleanup.disarm();
        pending_file = Some(pending);
        sync_directory(&parent)?;
        relative
    };
    insert_asset(transaction, &prepared.asset, &storage_rel_path)?;
    insert_asset_origin(transaction, &prepared.asset, origin)?;
    Ok((prepared.asset.clone(), pending_file))
}

fn insert_asset_origin(
    transaction: &Transaction<'_>,
    asset: &AssetRecord,
    origin: &AssetOrigin,
) -> Result<(), HostError> {
    let (origin, origin_ref) = match origin {
        AssetOrigin::User => (ASSET_ORIGIN_USER, None),
        AssetOrigin::BusinessDocument(generation_id) => {
            (ASSET_ORIGIN_BUSINESS_DOCUMENT, Some(generation_id.as_str()))
        }
        AssetOrigin::GeneratedArtifact { source, source_ref } => {
            (source.as_db_str(), Some(source_ref.as_str()))
        }
    };
    transaction
        .execute(
            "INSERT INTO asset_origins (asset_id, origin, origin_ref, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![asset.id, origin, origin_ref, asset.created_at],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn validate_prepared_asset_origin(
    asset: &AssetRecord,
    origin: &AssetOrigin,
) -> Result<(), HostError> {
    let AssetOrigin::GeneratedArtifact { source, .. } = origin else {
        return Ok(());
    };
    let valid = match source {
        GeneratedArtifactSource::ExtractionSnapshot => {
            asset.kind == AssetKind::Document && asset.mime_type == "application/json"
        }
        GeneratedArtifactSource::ReviewReport => asset.kind == AssetKind::Document,
        GeneratedArtifactSource::PagePreview => asset.kind == AssetKind::Image,
        GeneratedArtifactSource::ArchiveManifest => {
            asset.kind == AssetKind::Document && asset.mime_type == "application/json"
        }
        GeneratedArtifactSource::ArchivePackage => {
            asset.kind == AssetKind::Other && asset.mime_type == "application/zip"
        }
        GeneratedArtifactSource::NormalizedTemplate => {
            asset.kind == AssetKind::Document
                && asset.mime_type
                    == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                && Path::new(&asset.original_name)
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("docx"))
        }
    };
    if valid {
        Ok(())
    } else {
        Err(HostError::new(
            "GENERATED_ARTIFACT_TYPE_INVALID",
            format!(
                "{} does not accept detected type {}/{}",
                source.as_db_str(),
                asset_kind_to_db(&asset.kind),
                asset.mime_type
            ),
            false,
        ))
    }
}

fn mark_optional_committed(pending: &mut Option<PendingAssetFile>) {
    if let Some(pending) = pending.as_mut() {
        pending.mark_committed();
    }
}

fn preserve_optional_for_recovery(pending: &mut Option<PendingAssetFile>) {
    if let Some(pending) = pending.as_mut() {
        pending.preserve_for_recovery();
    }
}

fn cleanup_optional_uncommitted(pending: &mut Option<PendingAssetFile>) {
    if let Some(pending) = pending.as_mut() {
        pending.cleanup_uncommitted();
    }
}

fn settle_optional_for_persisted_asset(
    pending: &mut Option<PendingAssetFile>,
    candidate_asset_id: &str,
    persisted_asset_id: &str,
) {
    if candidate_asset_id == persisted_asset_id {
        mark_optional_committed(pending);
    } else {
        cleanup_optional_uncommitted(pending);
    }
}

fn insert_asset(
    transaction: &Transaction<'_>,
    asset: &AssetRecord,
    storage_rel_path: &str,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO assets
             (id, project_id, original_name, kind, mime_type, size_bytes, sha256, status,
              revision, created_at, updated_at, preview_available, storage_rel_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                asset.id,
                asset.project_id,
                asset.original_name,
                asset_kind_to_db(&asset.kind),
                asset.mime_type,
                asset.size_bytes,
                asset.sha256,
                asset_status_to_db(&asset.status),
                asset.revision,
                asset.created_at,
                asset.updated_at,
                i64::from(asset.preview_available),
                storage_rel_path,
            ],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn find_asset_by_origin_ref(
    connection: &Connection,
    origin: &str,
    origin_ref: &str,
) -> Result<Option<AssetRecord>, HostError> {
    connection
        .query_row(
            "SELECT asset.id, asset.project_id, asset.original_name, asset.kind, asset.mime_type,
                    asset.size_bytes, asset.sha256, asset.status, asset.revision, asset.created_at,
                    asset.updated_at, asset.preview_available
             FROM asset_origins origin
             JOIN assets asset ON asset.id = origin.asset_id
             WHERE origin.origin = ?1 AND origin.origin_ref = ?2",
            params![origin, origin_ref],
            asset_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn ensure_generated_artifact_project(
    asset: AssetRecord,
    expected_project_id: &str,
) -> Result<AssetRecord, HostError> {
    if asset.project_id.as_deref() == Some(expected_project_id) {
        Ok(asset)
    } else {
        Err(HostError::new(
            "GENERATED_ARTIFACT_SOURCE_CONFLICT",
            "generated artifact sourceRef already belongs to another project",
            false,
        ))
    }
}

fn find_asset(connection: &Connection, asset_id: &str) -> Result<Option<AssetRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, project_id, original_name, kind, mime_type, size_bytes, sha256,
                    status, revision, created_at, updated_at, preview_available
             FROM assets WHERE id = ?1",
            [asset_id],
            asset_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn find_reusable_storage(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    project_id: Option<&str>,
    sha256: &str,
    size_bytes: i64,
) -> Result<Option<String>, HostError> {
    let mut statement = transaction
        .prepare(
            "SELECT storage_rel_path
             FROM assets
             WHERE sha256 = ?1 AND size_bytes = ?2 AND status = 'ready'
               AND (project_id = ?3 OR (project_id IS NULL AND ?3 IS NULL))
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map(params![sha256, size_bytes, project_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(sql_error)?;
    for row in rows {
        let relative = row.map_err(sql_error)?;
        let reusable = resolve_existing_storage(vault_root, &relative)
            .ok()
            .and_then(|path| path.metadata().ok())
            .is_some_and(|metadata| metadata.len() == size_bytes as u64);
        if reusable {
            return Ok(Some(relative));
        }
    }
    Ok(None)
}

fn asset_source_from_row(row: &Row<'_>) -> rusqlite::Result<AssetSourceRecord> {
    let source_value: String = row.get(1)?;
    let source =
        asset_source_kind_from_db(&source_value).ok_or_else(|| conversion_error(&source_value))?;
    Ok(AssetSourceRecord {
        asset_id: row.get(0)?,
        source,
        source_ref: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn asset_source_kind_from_db(value: &str) -> Option<AssetSourceKind> {
    match value {
        ASSET_ORIGIN_USER => Some(AssetSourceKind::User),
        ASSET_ORIGIN_BUSINESS_DOCUMENT => Some(AssetSourceKind::BusinessDocument),
        ASSET_ORIGIN_GENERATED_EXTRACTION_SNAPSHOT => {
            Some(AssetSourceKind::GeneratedExtractionSnapshot)
        }
        ASSET_ORIGIN_GENERATED_REVIEW_REPORT => Some(AssetSourceKind::GeneratedReviewReport),
        ASSET_ORIGIN_GENERATED_PAGE_PREVIEW => Some(AssetSourceKind::GeneratedPagePreview),
        ASSET_ORIGIN_GENERATED_ARCHIVE_MANIFEST => Some(AssetSourceKind::ArchiveManifest),
        ASSET_ORIGIN_GENERATED_ARCHIVE_PACKAGE => Some(AssetSourceKind::ArchivePackage),
        ASSET_ORIGIN_NORMALIZED_TEMPLATE => Some(AssetSourceKind::NormalizedTemplate),
        _ => None,
    }
}

fn asset_from_row(row: &Row<'_>) -> rusqlite::Result<AssetRecord> {
    let kind_value: String = row.get(3)?;
    let kind = asset_kind_from_db(&kind_value).ok_or_else(|| conversion_error(&kind_value))?;
    let status_value: String = row.get(7)?;
    let status =
        asset_status_from_db(&status_value).ok_or_else(|| conversion_error(&status_value))?;
    let preview_available: i64 = row.get(11)?;
    Ok(AssetRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        original_name: row.get(2)?,
        kind,
        mime_type: row.get(4)?,
        size_bytes: row.get(5)?,
        sha256: row.get(6)?,
        status,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        preview_available: preview_available != 0,
    })
}

fn conversion_error(value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        value.len(),
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid asset database value: {value}"),
        )),
    )
}

fn asset_kind_to_db(kind: &AssetKind) -> &'static str {
    match kind {
        AssetKind::Image => "image",
        AssetKind::Video => "video",
        AssetKind::Audio => "audio",
        AssetKind::Document => "document",
        AssetKind::Other => "other",
    }
}

fn asset_kind_from_db(value: &str) -> Option<AssetKind> {
    Some(match value {
        "image" => AssetKind::Image,
        "video" => AssetKind::Video,
        "audio" => AssetKind::Audio,
        "document" => AssetKind::Document,
        "other" => AssetKind::Other,
        _ => return None,
    })
}

fn asset_status_to_db(status: &AssetStatus) -> &'static str {
    match status {
        AssetStatus::Ready => "ready",
        AssetStatus::Failed => "failed",
    }
}

fn asset_status_from_db(value: &str) -> Option<AssetStatus> {
    Some(match value {
        "ready" => AssetStatus::Ready,
        "failed" => AssetStatus::Failed,
        _ => return None,
    })
}

struct DetectedType {
    kind: AssetKind,
    mime_type: &'static str,
    extension: &'static str,
}

fn detected(kind: AssetKind, mime_type: &'static str, extension: &'static str) -> DetectedType {
    DetectedType {
        kind,
        mime_type,
        extension,
    }
}

fn detect_type(source_path: &Path, bytes: &[u8]) -> DetectedType {
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return detected(AssetKind::Image, "image/png", ".png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return detected(AssetKind::Image, "image/jpeg", ".jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return detected(AssetKind::Image, "image/gif", ".gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return detected(AssetKind::Image, "image/webp", ".webp");
    }
    if bytes.starts_with(b"BM") {
        return detected(AssetKind::Image, "image/bmp", ".bmp");
    }
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        return detected(AssetKind::Image, "image/tiff", ".tiff");
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if matches!(brand, b"avif" | b"avis") {
            return detected(AssetKind::Image, "image/avif", ".avif");
        }
        if matches!(
            brand,
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1"
        ) {
            return detected(AssetKind::Image, "image/heic", ".heic");
        }
        if matches!(extension.as_str(), "m4a" | "m4b" | "aac") {
            return detected(AssetKind::Audio, "audio/mp4", ".m4a");
        }
        if extension == "mov" || brand == b"qt  " {
            return detected(AssetKind::Video, "video/quicktime", ".mov");
        }
        return detected(AssetKind::Video, "video/mp4", ".mp4");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"AVI " {
        return detected(AssetKind::Video, "video/x-msvideo", ".avi");
    }
    if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        if extension == "webm" {
            return detected(AssetKind::Video, "video/webm", ".webm");
        }
        return detected(AssetKind::Video, "video/x-matroska", ".mkv");
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0xba]) || bytes.starts_with(&[0x00, 0x00, 0x01, 0xb3])
    {
        return detected(AssetKind::Video, "video/mpeg", ".mpeg");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE" {
        return detected(AssetKind::Audio, "audio/wav", ".wav");
    }
    if bytes.starts_with(b"fLaC") {
        return detected(AssetKind::Audio, "audio/flac", ".flac");
    }
    if bytes.starts_with(b"OggS") {
        if extension == "ogv" {
            return detected(AssetKind::Video, "video/ogg", ".ogv");
        }
        return detected(AssetKind::Audio, "audio/ogg", ".ogg");
    }
    if bytes.starts_with(b"ID3")
        || (bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0)
    {
        return detected(AssetKind::Audio, "audio/mpeg", ".mp3");
    }
    if bytes.starts_with(b"MThd") {
        return detected(AssetKind::Audio, "audio/midi", ".mid");
    }
    if bytes.starts_with(b"%PDF-") {
        return detected(AssetKind::Document, "application/pdf", ".pdf");
    }
    if bytes.starts_with(b"{\\rtf") {
        return detected(AssetKind::Document, "application/rtf", ".rtf");
    }
    if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) {
        return match extension.as_str() {
            "xls" => detected(AssetKind::Document, "application/vnd.ms-excel", ".xls"),
            "ppt" => detected(AssetKind::Document, "application/vnd.ms-powerpoint", ".ppt"),
            _ => detected(AssetKind::Document, "application/msword", ".doc"),
        };
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        return match extension.as_str() {
            "docx" => detected(
                AssetKind::Document,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                ".docx",
            ),
            "xlsx" => detected(
                AssetKind::Document,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                ".xlsx",
            ),
            "pptx" => detected(
                AssetKind::Document,
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                ".pptx",
            ),
            _ => detected(AssetKind::Other, "application/zip", ".zip"),
        };
    }

    if extension == "svg" {
        let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
        if text.contains("<svg") {
            return detected(AssetKind::Image, "image/svg+xml", ".svg");
        }
    }
    match extension.as_str() {
        "txt" | "md" | "log" | "srt" | "vtt" | "ass" => {
            detected(AssetKind::Document, "text/plain", ".txt")
        }
        "html" | "htm" => detected(AssetKind::Document, "text/html", ".html"),
        "csv" => detected(AssetKind::Document, "text/csv", ".csv"),
        "json" => detected(AssetKind::Document, "application/json", ".json"),
        "xml" => detected(AssetKind::Document, "application/xml", ".xml"),
        "yaml" | "yml" => detected(AssetKind::Document, "application/yaml", ".yaml"),
        _ => detected(AssetKind::Other, "application/octet-stream", ".bin"),
    }
}

fn storage_relative_path(sha256: &str, asset_id: &str, extension: &str) -> String {
    format!(
        "{}/{}/{}/original{}",
        &sha256[..2],
        &sha256[2..4],
        asset_id,
        extension
    )
}

fn prepare_vault_root(vault_root: &Path) -> Result<PathBuf, HostError> {
    fs::create_dir_all(vault_root).map_err(|error| vault_io_error("create Vault root", error))?;
    let metadata =
        fs::metadata(vault_root).map_err(|error| vault_io_error("inspect Vault root", error))?;
    if !metadata.is_dir() {
        return Err(HostError::new(
            "VAULT_INVALID",
            "Vault root is not a directory",
            false,
        ));
    }
    fs::canonicalize(vault_root).map_err(|error| vault_io_error("resolve Vault root", error))
}

fn join_relative_path(vault_root: &Path, relative: &str) -> Result<PathBuf, HostError> {
    let relative_path = Path::new(relative);
    if relative_path.as_os_str().is_empty()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(HostError::new(
            "VAULT_PATH_INVALID",
            "stored Vault path is not a safe relative path",
            false,
        ));
    }
    Ok(vault_root.join(relative_path))
}

fn resolve_existing_storage(vault_root: &Path, relative: &str) -> Result<PathBuf, HostError> {
    let candidate = join_relative_path(vault_root, relative)?;
    let resolved = fs::canonicalize(&candidate)
        .map_err(|error| vault_io_error("resolve Vault asset path", error))?;
    if !resolved.starts_with(vault_root) {
        return Err(HostError::new(
            "VAULT_PATH_INVALID",
            "stored Vault path escapes the Vault root",
            false,
        ));
    }
    let metadata = fs::metadata(&resolved)
        .map_err(|error| vault_io_error("inspect Vault asset file", error))?;
    if !metadata.is_file() {
        return Err(HostError::new(
            "VAULT_ASSET_MISSING",
            "stored Vault asset is not a regular file",
            true,
        ));
    }
    Ok(resolved)
}

fn original_name(source_path: &Path) -> Result<String, HostError> {
    let name = source_path
        .file_name()
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            HostError::new(
                "ASSET_SOURCE_INVALID",
                "asset source must have a file name",
                false,
            )
        })?;
    if name.chars().count() > 512 {
        return Err(HostError::validation(
            "asset original file name exceeds 512 characters",
        ));
    }
    Ok(name)
}

fn normalize_generated_artifact_ref(source_ref: &str) -> Result<String, HostError> {
    let normalized = source_ref.trim();
    let length = normalized.chars().count();
    if !(1..=MAX_GENERATED_ARTIFACT_REF_CHARS).contains(&length)
        || normalized.chars().any(char::is_control)
    {
        return Err(HostError::validation(
            "sourceRef must contain 1..256 non-control characters",
        ));
    }
    Ok(normalized.to_string())
}

fn normalize_project_id(project_id: Option<&str>) -> Result<Option<String>, HostError> {
    project_id
        .map(|value| {
            let normalized = value.trim();
            if normalized.is_empty() || normalized.chars().count() > 128 {
                return Err(HostError::validation(
                    "asset projectId must contain 1 to 128 characters",
                ));
            }
            Ok(normalized.to_string())
        })
        .transpose()
}

fn normalize_asset_id(asset_id: &str) -> Result<(), HostError> {
    Uuid::parse_str(asset_id)
        .map(|_| ())
        .map_err(|_| HostError::validation("assetId must be a UUID"))
}

fn validate_size(size: u64) -> Result<(), HostError> {
    if size > MAX_ASSET_SIZE_BYTES || size > i64::MAX as u64 {
        return Err(HostError::new(
            "ASSET_TOO_LARGE",
            format!(
                "asset exceeds the {} byte local import limit",
                MAX_ASSET_SIZE_BYTES
            ),
            false,
        ));
    }
    Ok(())
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::internal(format!("SQLite asset operation failed: {error}"))
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("JSON asset operation failed: {error}"))
}

fn source_io_error(action: &str, error: std::io::Error) -> HostError {
    HostError::new(
        "ASSET_SOURCE_IO",
        format!("{action} failed: {error}"),
        error.kind() != std::io::ErrorKind::NotFound,
    )
}

fn vault_io_error(action: &str, error: std::io::Error) -> HostError {
    HostError::new("VAULT_IO", format!("{action} failed: {error}"), true)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), HostError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| vault_io_error("sync Vault asset directory", error))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), HostError> {
    // The staged file itself is fsynced. Windows rename is atomic on this
    // same-volume path; opening directories for fsync needs platform flags.
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PendingImportIntent {
    version: u32,
    asset_id: String,
    sha256: String,
    extension: String,
    size_bytes: i64,
}

#[derive(Debug)]
struct StoredAssetStorage {
    storage_rel_path: String,
    sha256: String,
    size_bytes: i64,
}

struct PendingRecoveryIntentCreation {
    recovery_root: PathBuf,
    intent_path: PathBuf,
    file: Option<File>,
    armed: bool,
}

impl PendingRecoveryIntentCreation {
    fn new(recovery_root: PathBuf, intent_path: PathBuf, file: File) -> Self {
        Self {
            recovery_root,
            intent_path,
            file: Some(file),
            armed: true,
        }
    }

    fn file(&self) -> Result<&File, HostError> {
        self.file
            .as_ref()
            .ok_or_else(|| HostError::internal("asset recovery intent file is unavailable"))
    }

    fn file_mut(&mut self) -> Result<&mut File, HostError> {
        self.file
            .as_mut()
            .ok_or_else(|| HostError::internal("asset recovery intent file is unavailable"))
    }

    fn into_lease(mut self) -> Result<File, HostError> {
        let file = self
            .file
            .take()
            .ok_or_else(|| HostError::internal("asset recovery intent lease is unavailable"))?;
        self.armed = false;
        Ok(file)
    }
}

impl Drop for PendingRecoveryIntentCreation {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = FileExt::unlock(&file);
            drop(file);
        }
        if self.armed
            && remove_recovery_marker_if_safe(&self.recovery_root, &self.intent_path).is_ok()
        {
            let _ = sync_directory(&self.recovery_root);
        }
    }
}

struct PendingAssetFile {
    vault_root: PathBuf,
    recovery_root: PathBuf,
    final_path: PathBuf,
    intent_path: PathBuf,
    expected_size_bytes: i64,
    lease: Option<File>,
    armed: bool,
}

impl PendingAssetFile {
    fn mark_committed(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;
        self.release_lease();
        if remove_recovery_marker_if_safe(&self.recovery_root, &self.intent_path).is_ok() {
            let _ = sync_directory(&self.recovery_root);
        }
    }

    fn preserve_for_recovery(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;
        self.release_lease();
    }

    fn cleanup_uncommitted(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;
        let removed =
            remove_recovery_candidate(&self.vault_root, &self.final_path, self.expected_size_bytes)
                .is_ok();
        self.release_lease();
        if removed && remove_recovery_marker_if_safe(&self.recovery_root, &self.intent_path).is_ok()
        {
            let _ = sync_directory(&self.recovery_root);
        }
    }

    fn release_lease(&mut self) {
        if let Some(lease) = self.lease.take() {
            let _ = FileExt::unlock(&lease);
            drop(lease);
        }
    }
}

impl Drop for PendingAssetFile {
    fn drop(&mut self) {
        if self.armed {
            self.cleanup_uncommitted();
        } else {
            self.release_lease();
        }
    }
}

fn create_pending_asset_file(
    vault_root: &Path,
    final_path: PathBuf,
    asset: &AssetRecord,
    extension: &str,
) -> Result<PendingAssetFile, HostError> {
    let intent = PendingImportIntent {
        version: IMPORT_RECOVERY_INTENT_VERSION,
        asset_id: asset.id.clone(),
        sha256: asset.sha256.clone(),
        extension: extension.to_string(),
        size_bytes: asset.size_bytes,
    };
    validate_pending_import_intent_fields(&intent)?;

    let recovery_root = prepare_import_recovery_root(vault_root)?;
    let reconcile_lock =
        open_recovery_lock_file(&recovery_root.join(IMPORT_RECOVERY_LOCK_FILE), true)?
            .ok_or_else(|| HostError::internal("asset recovery lock was not created"))?;
    reconcile_lock
        .lock_exclusive()
        .map_err(|error| vault_io_error("lock asset recovery intent creation", error))?;

    let intent_path = recovery_root.join(format!(
        "{}{}",
        intent.asset_id, IMPORT_RECOVERY_INTENT_SUFFIX
    ));
    let intent_file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&intent_path)
        .map_err(|error| vault_io_error("create asset recovery intent", error))?;
    let mut creation =
        PendingRecoveryIntentCreation::new(recovery_root.clone(), intent_path.clone(), intent_file);
    validate_recovery_file_metadata(
        &creation
            .file()?
            .metadata()
            .map_err(|error| vault_io_error("inspect opened asset recovery intent", error))?,
        "asset recovery intent must be a regular file",
    )?;
    validate_recovery_marker_path(&recovery_root, &intent_path)?;
    creation
        .file()?
        .lock_exclusive()
        .map_err(|error| vault_io_error("lock asset recovery intent", error))?;

    let encoded = serde_json::to_vec(&intent)
        .map_err(|error| asset_recovery_error(format!("encode asset recovery intent: {error}")))?;
    if encoded.len() as u64 > MAX_IMPORT_RECOVERY_INTENT_BYTES {
        return Err(asset_recovery_error(
            "asset recovery intent exceeds the size limit",
        ));
    }
    let lease = creation.file_mut()?;
    lease
        .write_all(&encoded)
        .map_err(|error| vault_io_error("write asset recovery intent", error))?;
    lease
        .flush()
        .map_err(|error| vault_io_error("flush asset recovery intent", error))?;
    lease
        .sync_all()
        .map_err(|error| vault_io_error("sync asset recovery intent", error))?;
    sync_directory(&recovery_root)?;
    FileExt::unlock(&reconcile_lock)
        .map_err(|error| vault_io_error("unlock asset recovery intent creation", error))?;
    Ok(PendingAssetFile {
        vault_root: vault_root.to_path_buf(),
        recovery_root,
        final_path,
        intent_path,
        expected_size_bytes: asset.size_bytes,
        lease: Some(creation.into_lease()?),
        armed: true,
    })
}

fn prepare_import_recovery_root(vault_root: &Path) -> Result<PathBuf, HostError> {
    let candidate = vault_root.join(IMPORT_RECOVERY_DIRECTORY);
    match fs::symlink_metadata(&candidate) {
        Ok(metadata) => {
            if is_link_or_reparse(&metadata) || !metadata.is_dir() {
                return Err(vault_path_error(
                    "asset import recovery root must be a regular directory",
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&candidate)
                .map_err(|error| vault_io_error("create asset import recovery root", error))?;
            sync_directory(vault_root)?;
        }
        Err(error) => {
            return Err(vault_io_error("inspect asset import recovery root", error));
        }
    }
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| vault_io_error("inspect asset import recovery root", error))?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(vault_path_error(
            "asset import recovery root must be a regular directory",
        ));
    }
    let resolved = fs::canonicalize(&candidate)
        .map_err(|error| vault_io_error("resolve asset import recovery root", error))?;
    if resolved.parent() != Some(vault_root) || !resolved.starts_with(vault_root) {
        return Err(vault_path_error(
            "asset import recovery root escapes the Vault",
        ));
    }
    Ok(resolved)
}

fn open_recovery_lock_file(path: &Path, create: bool) -> Result<Option<File>, HostError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => validate_recovery_file_metadata(
            &metadata,
            "asset recovery lock must be a regular file",
        )?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => {
            return Ok(None);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(vault_io_error("inspect asset recovery lock", error)),
    }
    let file = match OpenOptions::new()
        .create(create)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => {
            return Ok(None);
        }
        Err(error) => return Err(vault_io_error("open asset recovery lock", error)),
    };
    validate_recovery_file_metadata(
        &file
            .metadata()
            .map_err(|error| vault_io_error("inspect opened asset recovery lock", error))?,
        "asset recovery lock must be a regular file",
    )?;
    validate_recovery_file_metadata(
        &fs::symlink_metadata(path)
            .map_err(|error| vault_io_error("inspect asset recovery lock path", error))?,
        "asset recovery lock must be a regular file",
    )?;
    Ok(Some(file))
}

fn validate_recovery_file_metadata(
    metadata: &fs::Metadata,
    message: &'static str,
) -> Result<(), HostError> {
    if is_link_or_reparse(metadata) || !metadata.is_file() {
        Err(vault_path_error(message))
    } else {
        Ok(())
    }
}

fn validate_recovery_marker_path(
    recovery_root: &Path,
    candidate: &Path,
) -> Result<PathBuf, HostError> {
    if candidate.parent() != Some(recovery_root) {
        return Err(vault_path_error(
            "asset recovery intent is outside the recovery root",
        ));
    }
    let metadata = fs::symlink_metadata(candidate)
        .map_err(|error| vault_io_error("inspect asset recovery intent path", error))?;
    validate_recovery_file_metadata(&metadata, "asset recovery intent must be a regular file")?;
    let resolved = fs::canonicalize(candidate)
        .map_err(|error| vault_io_error("resolve asset recovery intent", error))?;
    if resolved.parent() != Some(recovery_root) || !resolved.starts_with(recovery_root) {
        return Err(vault_path_error(
            "asset recovery intent escapes the recovery root",
        ));
    }
    Ok(resolved)
}

fn remove_recovery_marker_if_safe(recovery_root: &Path, candidate: &Path) -> Result<(), HostError> {
    match fs::symlink_metadata(candidate) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(vault_io_error("inspect asset recovery intent", error)),
    }
    let resolved = validate_recovery_marker_path(recovery_root, candidate)?;
    fs::remove_file(resolved).map_err(|error| vault_io_error("remove asset recovery intent", error))
}

fn prepare_internal_asset_directory(vault_root: &Path, directory: &Path) -> Result<(), HostError> {
    let relative = directory
        .strip_prefix(vault_root)
        .map_err(|_| vault_path_error("Vault asset directory escapes the Vault"))?;
    let mut current = vault_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(vault_path_error("Vault asset directory is not a safe path"));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if is_link_or_reparse(&metadata) || !metadata.is_dir() {
                    return Err(vault_path_error(
                        "Vault asset directory must be a regular directory",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let parent = current
                    .parent()
                    .ok_or_else(|| vault_path_error("Vault asset directory has no parent"))?;
                fs::create_dir(&current)
                    .map_err(|error| vault_io_error("create Vault asset directory", error))?;
                sync_directory(parent)?;
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|error| vault_io_error("inspect Vault asset directory", error))?;
                if is_link_or_reparse(&metadata) || !metadata.is_dir() {
                    return Err(vault_path_error(
                        "Vault asset directory must be a regular directory",
                    ));
                }
            }
            Err(error) => {
                return Err(vault_io_error("inspect Vault asset directory", error));
            }
        }
        let resolved = fs::canonicalize(&current)
            .map_err(|error| vault_io_error("resolve Vault asset directory", error))?;
        if !resolved.starts_with(vault_root) {
            return Err(vault_path_error("Vault asset directory escapes the Vault"));
        }
    }
    Ok(())
}

fn validate_internal_asset_parent(vault_root: &Path, parent: &Path) -> Result<(), HostError> {
    let relative = parent
        .strip_prefix(vault_root)
        .map_err(|_| vault_path_error("Vault asset directory escapes the Vault"))?;
    let mut current = vault_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(vault_path_error("Vault asset directory is not a safe path"));
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| vault_io_error("inspect Vault asset directory", error))?;
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(vault_path_error(
                "Vault asset directory must be a regular directory",
            ));
        }
        let resolved = fs::canonicalize(&current)
            .map_err(|error| vault_io_error("resolve Vault asset directory", error))?;
        if !resolved.starts_with(vault_root) {
            return Err(vault_path_error("Vault asset directory escapes the Vault"));
        }
    }
    Ok(())
}

fn validate_recovery_candidate(
    vault_root: &Path,
    candidate: &Path,
    expected_size_bytes: i64,
) -> Result<PathBuf, HostError> {
    let parent = candidate
        .parent()
        .ok_or_else(|| vault_path_error("recovered Vault asset has no parent directory"))?;
    validate_internal_asset_parent(vault_root, parent)?;
    let metadata = fs::symlink_metadata(candidate)
        .map_err(|error| vault_io_error("inspect recovered Vault asset", error))?;
    if is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(vault_path_error(
            "recovered Vault asset must be a regular file",
        ));
    }
    if expected_size_bytes < 0 || metadata.len() != expected_size_bytes as u64 {
        return Err(asset_recovery_error(
            "recovered Vault asset size does not match its intent",
        ));
    }
    let resolved = fs::canonicalize(candidate)
        .map_err(|error| vault_io_error("resolve recovered Vault asset", error))?;
    if !resolved.starts_with(vault_root) {
        return Err(vault_path_error("recovered Vault asset escapes the Vault"));
    }
    Ok(resolved)
}

fn remove_recovery_candidate(
    vault_root: &Path,
    candidate: &Path,
    expected_size_bytes: i64,
) -> Result<(), HostError> {
    match fs::symlink_metadata(candidate) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(vault_io_error("inspect recovered Vault asset", error)),
    }
    let resolved = validate_recovery_candidate(vault_root, candidate, expected_size_bytes)?;
    let parent = resolved
        .parent()
        .ok_or_else(|| vault_path_error("recovered Vault asset has no parent directory"))?
        .to_path_buf();
    fs::remove_file(&resolved)
        .map_err(|error| vault_io_error("remove uncommitted Vault asset", error))?;
    // The recovery marker is only cleared after the candidate directory records
    // the unlink. On platforms where directory fsync is unavailable this remains
    // best-effort and a stale marker is harmless on the next reconciliation.
    sync_directory(&parent)
}

fn validate_pending_import_intent(
    intent: &PendingImportIntent,
    intent_path: &Path,
) -> Result<(), HostError> {
    validate_pending_import_intent_fields(intent)?;
    let expected_name = format!("{}{}", intent.asset_id, IMPORT_RECOVERY_INTENT_SUFFIX);
    if intent_path.file_name().and_then(|value| value.to_str()) != Some(expected_name.as_str()) {
        return Err(asset_recovery_error(
            "asset recovery intent filename does not match its assetId",
        ));
    }
    Ok(())
}

fn validate_pending_import_intent_fields(intent: &PendingImportIntent) -> Result<(), HostError> {
    if intent.version != IMPORT_RECOVERY_INTENT_VERSION {
        return Err(asset_recovery_error(
            "asset recovery intent version is unsupported",
        ));
    }
    let normalized_id = Uuid::parse_str(&intent.asset_id)
        .map(|value| value.to_string())
        .map_err(|_| asset_recovery_error("asset recovery assetId must be a UUID"))?;
    if normalized_id != intent.asset_id {
        return Err(asset_recovery_error(
            "asset recovery assetId must use canonical UUID form",
        ));
    }
    if intent.sha256.len() != 64
        || !intent
            .sha256
            .bytes()
            .all(|value| value.is_ascii_digit() || matches!(value, b'a'..=b'f'))
    {
        return Err(asset_recovery_error(
            "asset recovery sha256 must be 64 lowercase hexadecimal characters",
        ));
    }
    if intent.extension.len() < 2
        || intent.extension.len() > 16
        || !intent.extension.starts_with('.')
        || !intent.extension[1..]
            .bytes()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit())
    {
        return Err(asset_recovery_error("asset recovery extension is invalid"));
    }
    if intent.size_bytes < 0 || intent.size_bytes as u64 > MAX_ASSET_SIZE_BYTES {
        return Err(asset_recovery_error(
            "asset recovery size is outside the supported range",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct StorageReferenceSummary {
    total: i64,
    matching: i64,
}

fn summarize_storage_references(
    connection: &Connection,
    storage_rel_path: &str,
    sha256: &str,
    size_bytes: i64,
) -> Result<StorageReferenceSummary, HostError> {
    connection
        .query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN sha256 = ?2 AND size_bytes = ?3 THEN 1 ELSE 0 END), 0)
             FROM assets WHERE storage_rel_path = ?1",
            params![storage_rel_path, sha256, size_bytes],
            |row| {
                Ok(StorageReferenceSummary {
                    total: row.get(0)?,
                    matching: row.get(1)?,
                })
            },
        )
        .map_err(sql_error)
}
fn find_asset_storage(
    connection: &Connection,
    asset_id: &str,
) -> Result<Option<StoredAssetStorage>, HostError> {
    connection
        .query_row(
            "SELECT storage_rel_path, sha256, size_bytes FROM assets WHERE id = ?1",
            [asset_id],
            |row| {
                Ok(StoredAssetStorage {
                    storage_rel_path: row.get(0)?,
                    sha256: row.get(1)?,
                    size_bytes: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(sql_error)
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn is_lock_contended(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33))
}

fn vault_path_error(message: impl Into<String>) -> HostError {
    HostError::new("VAULT_PATH_INVALID", message, false)
}

fn asset_recovery_error(message: impl Into<String>) -> HostError {
    HostError::new("ASSET_RECOVERY_INVALID", message, false)
}

struct CleanupFile {
    path: PathBuf,
    armed: bool,
}

impl CleanupFile {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CleanupFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ImportAssetPayload, OperationContext};
    use tempfile::tempdir;

    fn source(directory: &Path, name: &str, contents: &[u8]) -> PathBuf {
        let path = directory.join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    fn file_count(path: &Path) -> usize {
        if !path.exists() {
            return 0;
        }
        fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .map(|entry| {
                let internal_directory = entry.is_dir()
                    && entry.file_name().is_some_and(|name| {
                        name == ".staging" || name == IMPORT_RECOVERY_DIRECTORY
                    });
                if internal_directory {
                    0
                } else if entry.is_dir() {
                    file_count(&entry)
                } else {
                    1
                }
            })
            .sum()
    }

    fn recovery_intent_count(vault_root: &Path) -> usize {
        let recovery_root = vault_root.join(IMPORT_RECOVERY_DIRECTORY);
        if !recovery_root.exists() {
            return 0;
        }
        fs::read_dir(recovery_root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with(IMPORT_RECOVERY_INTENT_SUFFIX)
            })
            .count()
    }

    fn persist_pending_import(
        connection: &mut Connection,
        vault_root: &Path,
        source_path: &Path,
        commit: bool,
    ) -> (AssetRecord, PathBuf, PendingAssetFile) {
        let mut prepared =
            prepare_import(vault_root, Some("project-recovery"), source_path).unwrap();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let (asset, pending) =
            persist_prepared_asset(&transaction, &mut prepared, &AssetOrigin::User).unwrap();
        let pending = pending.expect("new test content must create a physical candidate");
        let final_path = pending.final_path.clone();
        if commit {
            transaction.commit().unwrap();
        } else {
            transaction.rollback().unwrap();
        }
        (asset, final_path, pending)
    }

    fn import_command(
        command_id: &str,
        idempotency_key: &str,
        source_token: &str,
        project_id: Option<&str>,
        deadline_at: Option<i64>,
    ) -> AssetCommandEnvelope {
        AssetCommandEnvelope::Import {
            command_id: command_id.to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            context: OperationContext {
                actor_id: "operator-local".to_string(),
                account_id: None,
                project_id: project_id.map(str::to_string),
                window_id: "main".to_string(),
                trace_id: format!("trace-{command_id}"),
            },
            payload: ImportAssetPayload {
                source_token: source_token.to_string(),
                project_id: project_id.map(str::to_string),
            },
            idempotency_key: idempotency_key.to_string(),
            expected_revision: None,
            deadline_at,
        }
    }

    #[test]
    fn migration_backfills_existing_assets_with_user_provenance() {
        let connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        connection.execute("DROP TABLE asset_origins", []).unwrap();
        let asset_id = Uuid::new_v4().to_string();
        connection
            .execute(
                "INSERT INTO assets
                 (id, project_id, original_name, kind, mime_type, size_bytes, sha256,
                  status, revision, created_at, updated_at, preview_available, storage_rel_path)
                 VALUES (?1, NULL, 'legacy.pdf', 'document', 'application/pdf', 1, ?2,
                         'ready', 1, 1, 1, 0, 'objects/aa/bb/legacy.pdf')",
                params![asset_id, "a".repeat(64)],
            )
            .unwrap();

        migrate(&connection).unwrap();
        let (origin, origin_ref): (String, Option<String>) = connection
            .query_row(
                "SELECT origin, origin_ref FROM asset_origins WHERE asset_id = ?1",
                [&asset_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(origin, "user");
        assert_eq!(origin_ref, None);
    }

    #[test]
    fn migration_adds_normalized_template_origin_and_preserves_existing_sources() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let user_source = source(directory.path(), "legacy-user.txt", b"legacy user");
        let business_source = source(directory.path(), "legacy-business.pdf", b"%PDF-1.4\nlegacy");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        let user_asset = import_file(
            &mut connection,
            &vault,
            Some("project-legacy"),
            &user_source,
        )
        .unwrap();
        let generation_id = Uuid::new_v4().to_string();
        let business_asset = import_business_document(
            &mut connection,
            &vault,
            "project-legacy",
            &business_source,
            &generation_id,
        )
        .unwrap();
        let report_source = source(directory.path(), "review.html", b"<html>review</html>");
        let report = import_generated_artifact(
            &mut connection,
            &vault,
            "project-legacy",
            &report_source,
            GeneratedArtifactSource::ReviewReport,
            "review-session-legacy",
        )
        .unwrap();
        connection
            .execute_batch(
                r#"
                DROP INDEX idx_asset_origins_business_ref;
                ALTER TABLE asset_origins RENAME TO asset_origins_current;
                CREATE TABLE asset_origins (
                    asset_id TEXT PRIMARY KEY NOT NULL,
                    origin TEXT NOT NULL CHECK(origin IN (
                        'user',
                        'businessDocument',
                        'generatedExtractionSnapshot',
                        'generatedReviewReport',
                        'generatedPagePreview',
                        'generatedArchiveManifest',
                        'generatedArchivePackage'
                    )),
                    origin_ref TEXT,
                    created_at INTEGER NOT NULL,
                    CHECK(
                        (origin = 'user' AND origin_ref IS NULL)
                        OR (
                            origin IN (
                                'businessDocument',
                                'generatedExtractionSnapshot',
                                'generatedReviewReport',
                                'generatedPagePreview',
                                'generatedArchiveManifest',
                                'generatedArchivePackage'
                            )
                            AND origin_ref IS NOT NULL
                        )
                    ),
                    FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
                );
                INSERT INTO asset_origins (asset_id, origin, origin_ref, created_at)
                    SELECT asset_id, origin, origin_ref, created_at FROM asset_origins_current;
                DROP TABLE asset_origins_current;
                CREATE UNIQUE INDEX idx_asset_origins_business_ref
                    ON asset_origins(origin, origin_ref) WHERE origin_ref IS NOT NULL;
                "#,
            )
            .unwrap();

        migrate(&connection).unwrap();
        migrate(&connection).unwrap();
        assert_eq!(
            get_asset_source(&connection, &user_asset.id)
                .unwrap()
                .source,
            AssetSourceKind::User
        );
        let business_origin = get_asset_source(&connection, &business_asset.id).unwrap();
        assert_eq!(business_origin.source, AssetSourceKind::BusinessDocument);
        assert_eq!(
            business_origin.source_ref.as_deref(),
            Some(generation_id.as_str())
        );
        assert_eq!(
            get_asset_source(&connection, &report.id).unwrap().source,
            AssetSourceKind::GeneratedReviewReport
        );
        let schema: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'asset_origins'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(schema.contains(ASSET_ORIGIN_GENERATED_ARCHIVE_MANIFEST));
        assert!(schema.contains(ASSET_ORIGIN_GENERATED_ARCHIVE_PACKAGE));
        assert!(schema.contains(ASSET_ORIGIN_NORMALIZED_TEMPLATE));
        let source_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM asset_origins", [], |row| row.get(0))
            .unwrap();
        assert_eq!(source_count, 3);
    }

    #[test]
    fn normalized_template_origin_ref_is_idempotent() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let first_source = source(
            directory.path(),
            "normalized-template.docx",
            b"PK\x03\x04normalized-template-v1",
        );
        let replacement_source = source(
            directory.path(),
            "replacement-template.docx",
            b"PK\x03\x04different-template-bytes",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let imported = import_generated_artifact(
            &mut connection,
            &vault,
            "project-template",
            &first_source,
            GeneratedArtifactSource::NormalizedTemplate,
            "doc-normalize:v1:source-asset:source-sha:word:policy-v1",
        )
        .unwrap();
        let replayed = import_generated_artifact(
            &mut connection,
            &vault,
            "project-template",
            &replacement_source,
            GeneratedArtifactSource::NormalizedTemplate,
            "doc-normalize:v1:source-asset:source-sha:word:policy-v1",
        )
        .unwrap();

        assert_eq!(replayed, imported);
        assert_eq!(list_assets(&connection, None).unwrap().len(), 1);
        let origin = get_asset_source(&connection, &imported.id).unwrap();
        assert_eq!(origin.source, AssetSourceKind::NormalizedTemplate);
        assert_eq!(
            origin.source_ref.as_deref(),
            Some("doc-normalize:v1:source-asset:source-sha:word:policy-v1")
        );
    }

    #[test]
    fn normalized_template_rejects_non_docx_outputs() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let pdf_source = source(
            directory.path(),
            "normalized-template.docx",
            b"%PDF-1.7\nnot-a-docx",
        );
        let xlsx_source = source(
            directory.path(),
            "normalized-template.xlsx",
            b"PK\x03\x04spreadsheet-package",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        for (source_path, source_ref) in [
            (pdf_source.as_path(), "normalized-pdf"),
            (xlsx_source.as_path(), "normalized-xlsx"),
        ] {
            let error = import_generated_artifact(
                &mut connection,
                &vault,
                "project-template",
                source_path,
                GeneratedArtifactSource::NormalizedTemplate,
                source_ref,
            )
            .unwrap_err();
            assert_eq!(error.code, "GENERATED_ARTIFACT_TYPE_INVALID");
        }
        assert!(list_assets(&connection, None).unwrap().is_empty());
        assert_eq!(file_count(&vault), 0);
        assert_eq!(recovery_intent_count(&vault), 0);
    }

    #[test]
    fn normalized_template_rejects_cross_project_origin_ref() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let normalized_source = source(
            directory.path(),
            "normalized-template.docx",
            b"PK\x03\x04normalized-template",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        import_generated_artifact(
            &mut connection,
            &vault,
            "project-a",
            &normalized_source,
            GeneratedArtifactSource::NormalizedTemplate,
            "shared-normalization-identity",
        )
        .unwrap();
        let error = import_generated_artifact(
            &mut connection,
            &vault,
            "project-b",
            &normalized_source,
            GeneratedArtifactSource::NormalizedTemplate,
            "shared-normalization-identity",
        )
        .unwrap_err();

        assert_eq!(error.code, "GENERATED_ARTIFACT_SOURCE_CONFLICT");
        assert_eq!(list_assets(&connection, None).unwrap().len(), 1);
    }

    #[test]
    fn normalized_template_orphan_is_reconciled_after_restart() {
        let directory = tempdir().unwrap();
        let database_path = directory.path().join("assets.sqlite3");
        let vault = directory.path().join("vault");
        let normalized_source = source(
            directory.path(),
            "normalized-template.docx",
            b"PK\x03\x04restart-recovery-template",
        );

        let orphan_path = {
            let mut connection = Connection::open(&database_path).unwrap();
            migrate(&connection).unwrap();
            let origin = AssetOrigin::GeneratedArtifact {
                source: GeneratedArtifactSource::NormalizedTemplate,
                source_ref: "restart-normalized-template".to_string(),
            };
            let mut prepared =
                prepare_import(&vault, Some("project-restart"), &normalized_source).unwrap();
            validate_prepared_asset_origin(&prepared.asset, &origin).unwrap();
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            let (_asset, pending) =
                persist_prepared_asset(&transaction, &mut prepared, &origin).unwrap();
            let mut pending = pending.expect("new template must create a Vault candidate");
            let orphan_path = pending.final_path.clone();
            transaction.rollback().unwrap();
            pending.preserve_for_recovery();
            assert!(orphan_path.exists());
            assert_eq!(recovery_intent_count(&vault), 1);
            orphan_path
        };

        let connection = Connection::open(&database_path).unwrap();
        migrate(&connection).unwrap();
        reconcile_pending_imports(&connection, &vault).unwrap();

        assert!(!orphan_path.exists());
        assert!(list_assets(&connection, None).unwrap().is_empty());
        assert_eq!(recovery_intent_count(&vault), 0);
        reconcile_pending_imports(&connection, &vault).unwrap();
        assert_eq!(recovery_intent_count(&vault), 0);
    }

    #[test]
    fn generated_artifacts_are_typed_queryable_and_idempotent() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let snapshot_source = source(
            directory.path(),
            "extraction.json",
            br#"{"pages":[{"text":"contract"}]}"#,
        );
        let report_source = source(directory.path(), "review.html", b"<html>review</html>");
        let preview_source = source(directory.path(), "page-1.png", b"\x89PNG\r\n\x1a\npreview");
        let manifest_source = source(
            directory.path(),
            "archive-manifest.json",
            br#"{"version":1,"entries":[]}"#,
        );
        let package_source = source(directory.path(), "archive.zip", b"PK\x05\x06empty-archive");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let cases = [
            (
                GeneratedArtifactSource::ExtractionSnapshot,
                "extraction-v1",
                snapshot_source.as_path(),
                AssetSourceKind::GeneratedExtractionSnapshot,
                ASSET_ORIGIN_GENERATED_EXTRACTION_SNAPSHOT,
            ),
            (
                GeneratedArtifactSource::ReviewReport,
                "review-v1",
                report_source.as_path(),
                AssetSourceKind::GeneratedReviewReport,
                ASSET_ORIGIN_GENERATED_REVIEW_REPORT,
            ),
            (
                GeneratedArtifactSource::PagePreview,
                "page-preview-v1-page-1",
                preview_source.as_path(),
                AssetSourceKind::GeneratedPagePreview,
                ASSET_ORIGIN_GENERATED_PAGE_PREVIEW,
            ),
            (
                GeneratedArtifactSource::ArchiveManifest,
                "archive-manifest-v1",
                manifest_source.as_path(),
                AssetSourceKind::ArchiveManifest,
                ASSET_ORIGIN_GENERATED_ARCHIVE_MANIFEST,
            ),
            (
                GeneratedArtifactSource::ArchivePackage,
                "archive-package-v1",
                package_source.as_path(),
                AssetSourceKind::ArchivePackage,
                ASSET_ORIGIN_GENERATED_ARCHIVE_PACKAGE,
            ),
        ];
        let mut imported = Vec::new();
        for (generated_source, source_ref, source_path, expected_source, expected_db_origin) in
            cases
        {
            let asset = import_generated_artifact(
                &mut connection,
                &vault,
                "project-contract",
                source_path,
                generated_source,
                source_ref,
            )
            .unwrap();
            assert_eq!(asset.project_id.as_deref(), Some("project-contract"));
            assert!(resolve_original_path(&connection, &vault, &asset.id)
                .unwrap()
                .exists());
            let origin = get_asset_source(&connection, &asset.id).unwrap();
            assert_eq!(origin.asset_id, asset.id);
            assert_eq!(origin.source, expected_source);
            assert_eq!(origin.source_ref.as_deref(), Some(source_ref));
            let db_origin: String = connection
                .query_row(
                    "SELECT origin FROM asset_origins WHERE asset_id = ?1",
                    [&asset.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(db_origin, expected_db_origin);
            imported.push(asset);
        }

        fs::remove_file(&report_source).unwrap();
        let replayed = import_generated_artifact(
            &mut connection,
            &vault,
            "project-contract",
            &report_source,
            GeneratedArtifactSource::ReviewReport,
            "review-v1",
        )
        .unwrap();
        assert_eq!(replayed.id, imported[1].id);
        assert_eq!(
            list_assets(&connection, Some("project-contract"))
                .unwrap()
                .len(),
            5
        );
    }

    #[test]
    fn generated_artifact_type_and_project_conflicts_are_rejected() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let json_source = source(directory.path(), "snapshot.json", br#"{"ok":true}"#);
        let image_source = source(directory.path(), "preview.png", b"\x89PNG\r\n\x1a\npreview");
        let zip_source = source(directory.path(), "archive.zip", b"PK\x05\x06empty-archive");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let error = import_generated_artifact(
            &mut connection,
            &vault,
            "project-a",
            &json_source,
            GeneratedArtifactSource::PagePreview,
            "preview-wrong-type",
        )
        .unwrap_err();
        assert_eq!(error.code, "GENERATED_ARTIFACT_TYPE_INVALID");
        assert!(list_assets(&connection, None).unwrap().is_empty());

        let error = import_generated_artifact(
            &mut connection,
            &vault,
            "project-a",
            &json_source,
            GeneratedArtifactSource::ArchivePackage,
            "archive-package-wrong-type",
        )
        .unwrap_err();
        assert_eq!(error.code, "GENERATED_ARTIFACT_TYPE_INVALID");

        let error = import_generated_artifact(
            &mut connection,
            &vault,
            "project-a",
            &zip_source,
            GeneratedArtifactSource::ArchiveManifest,
            "archive-manifest-wrong-type",
        )
        .unwrap_err();
        assert_eq!(error.code, "GENERATED_ARTIFACT_TYPE_INVALID");
        assert!(list_assets(&connection, None).unwrap().is_empty());

        import_generated_artifact(
            &mut connection,
            &vault,
            "project-a",
            &image_source,
            GeneratedArtifactSource::PagePreview,
            "preview-shared-ref",
        )
        .unwrap();
        let error = import_generated_artifact(
            &mut connection,
            &vault,
            "project-b",
            &image_source,
            GeneratedArtifactSource::PagePreview,
            "preview-shared-ref",
        )
        .unwrap_err();
        assert_eq!(error.code, "GENERATED_ARTIFACT_SOURCE_CONFLICT");
        assert_eq!(list_assets(&connection, None).unwrap().len(), 1);
    }

    #[test]
    fn imports_hashes_and_never_exposes_paths() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault-private");
        let source = source(directory.path(), "sample.txt", b"hello world\n");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let asset = import_file(&mut connection, &vault, Some("project-alpha"), &source).unwrap();

        assert_eq!(asset.original_name, "sample.txt");
        assert_eq!(asset.project_id.as_deref(), Some("project-alpha"));
        assert_eq!(asset.size_bytes, 12);
        assert_eq!(
            asset.sha256,
            "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447"
        );
        assert_eq!(asset.kind, AssetKind::Document);
        assert_eq!(asset.mime_type, "text/plain");
        assert_eq!(asset.status, AssetStatus::Ready);
        assert_eq!(
            fs::read(resolve_original_path(&connection, &vault, &asset.id).unwrap()).unwrap(),
            b"hello world\n"
        );

        let wire = serde_json::to_string(&asset).unwrap();
        assert!(!wire.contains(&source.to_string_lossy().to_string()));
        assert!(!wire.contains(&vault.to_string_lossy().to_string()));
        assert!(!wire.contains("storage_rel_path"));
        assert!(!wire.contains("storageRelPath"));
    }

    #[test]
    fn native_asset_access_verifies_integrity_and_exports_without_paths() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault-private");
        let source = source(directory.path(), "contract.pdf", b"verified-contract-bytes");
        let destination = directory.path().join("exported-contract.pdf");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let asset = import_file(&mut connection, &vault, Some("project-alpha"), &source).unwrap();
        let (verified, native_path) =
            verify_ready_asset_integrity(&connection, &vault, &asset.id).unwrap();
        assert_eq!(verified, asset);
        assert!(native_path.starts_with(fs::canonicalize(&vault).unwrap()));

        export_verified_asset_to_path(&connection, &vault, &asset.id, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"verified-contract-bytes");

        let inside_vault = vault.join("forbidden-export.pdf");
        let error = export_verified_asset_to_path(&connection, &vault, &asset.id, &inside_vault)
            .unwrap_err();
        assert_eq!(error.code, "ASSET_EXPORT_TARGET_INVALID");
    }

    #[test]
    fn native_asset_access_rejects_tampered_vault_bytes_before_export() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault-private");
        let source = source(directory.path(), "contract.docx", b"authoritative-contract");
        let destination = directory.path().join("existing-export.docx");
        fs::write(&destination, b"preserve-existing-output").unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let asset = import_file(&mut connection, &vault, Some("project-alpha"), &source).unwrap();
        let native_path = resolve_original_path(&connection, &vault, &asset.id).unwrap();
        fs::write(native_path, b"tampered-contract-bytes").unwrap();

        let error = verify_ready_asset_integrity(&connection, &vault, &asset.id).unwrap_err();
        assert_eq!(error.code, "VAULT_ASSET_INTEGRITY_MISMATCH");
        let error = export_verified_asset_to_path(&connection, &vault, &asset.id, &destination)
            .unwrap_err();
        assert_eq!(error.code, "VAULT_ASSET_INTEGRITY_MISMATCH");
        assert_eq!(fs::read(destination).unwrap(), b"preserve-existing-output");
    }

    #[test]
    fn template_asset_read_returns_verified_record_and_bytes() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault-private");
        let source = source(
            directory.path(),
            "contract-template.docx",
            b"verified-template-bytes",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let asset = import_file(&mut connection, &vault, Some("project-alpha"), &source).unwrap();
        let (verified, bytes) =
            read_verified_template_asset(&connection, &vault, &asset.id).unwrap();

        assert_eq!(verified, asset);
        assert_eq!(bytes, b"verified-template-bytes");
    }

    #[test]
    fn bounded_asset_read_returns_verified_record_and_bytes() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault-private");
        let source = source(directory.path(), "screenshot.png", b"verified-image-bytes");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let asset = import_file(&mut connection, &vault, Some("project-alpha"), &source).unwrap();
        let (verified, bytes) =
            read_verified_asset_limited(&connection, &vault, &asset.id, 1024).unwrap();

        assert_eq!(verified, asset);
        assert_eq!(bytes, b"verified-image-bytes");
    }

    #[test]
    fn bounded_asset_read_rejects_invalid_or_exceeded_limits() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault-private");
        let source = source(directory.path(), "screenshot.png", b"verified-image-bytes");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let asset = import_file(&mut connection, &vault, Some("project-alpha"), &source).unwrap();
        let error = read_verified_asset_limited(&connection, &vault, &asset.id, 0).unwrap_err();
        assert_eq!(error.code, "ASSET_READ_LIMIT_INVALID");

        let error = read_verified_asset_limited(&connection, &vault, &asset.id, 4).unwrap_err();
        assert_eq!(error.code, "ASSET_TOO_LARGE");
    }

    #[test]
    fn template_asset_read_rejects_same_size_tampering() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault-private");
        let source = source(
            directory.path(),
            "service-template.docx",
            b"authoritative-template",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let asset = import_file(&mut connection, &vault, Some("project-alpha"), &source).unwrap();
        let native_path = resolve_original_path(&connection, &vault, &asset.id).unwrap();
        fs::write(native_path, b"tampered-template-byte").unwrap();

        let error = read_verified_template_asset(&connection, &vault, &asset.id).unwrap_err();
        assert_eq!(error.code, "VAULT_ASSET_INTEGRITY_MISMATCH");
    }

    #[test]
    fn template_asset_read_rejects_assets_over_safety_limit() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault-private");
        let source = source(
            directory.path(),
            "oversized-template.xlsx",
            b"small-template",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let asset = import_file(&mut connection, &vault, Some("project-alpha"), &source).unwrap();
        let native_path = resolve_original_path(&connection, &vault, &asset.id).unwrap();
        OpenOptions::new()
            .write(true)
            .open(native_path)
            .unwrap()
            .set_len(MAX_TEMPLATE_ASSET_SIZE_BYTES + 1)
            .unwrap();
        connection
            .execute(
                "UPDATE assets SET size_bytes = ?1 WHERE id = ?2",
                params![MAX_TEMPLATE_ASSET_SIZE_BYTES as i64 + 1, asset.id],
            )
            .unwrap();

        let error = read_verified_template_asset(&connection, &vault, &asset.id).unwrap_err();
        assert_eq!(error.code, "TEMPLATE_ASSET_TOO_LARGE");
    }

    #[test]
    fn survives_database_reopen() {
        let directory = tempdir().unwrap();
        let database = directory.path().join("asset.sqlite3");
        let vault = directory.path().join("vault");
        let source = source(directory.path(), "recover.pdf", b"%PDF-1.7\nrecovery");
        let asset_id;
        {
            let mut connection = Connection::open(&database).unwrap();
            migrate(&connection).unwrap();
            asset_id = import_file(&mut connection, &vault, None, &source)
                .unwrap()
                .id;
        }

        let connection = Connection::open(&database).unwrap();
        migrate(&connection).unwrap();
        let recovered = get_asset(&connection, &asset_id).unwrap();
        assert_eq!(recovered.original_name, "recover.pdf");
        assert_eq!(recovered.mime_type, "application/pdf");
        assert!(resolve_original_path(&connection, &vault, &asset_id)
            .unwrap()
            .is_file());
    }

    #[test]
    fn keeps_same_names_as_distinct_logical_assets() {
        let directory = tempdir().unwrap();
        let first_directory = directory.path().join("first");
        let second_directory = directory.path().join("second");
        fs::create_dir_all(&first_directory).unwrap();
        fs::create_dir_all(&second_directory).unwrap();
        let first = source(&first_directory, "shot.png", b"first content");
        let second = source(&second_directory, "shot.png", b"second content");
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let first_asset = import_file(&mut connection, &vault, Some("project"), &first).unwrap();
        let second_asset = import_file(&mut connection, &vault, Some("project"), &second).unwrap();

        assert_ne!(first_asset.id, second_asset.id);
        assert_ne!(first_asset.sha256, second_asset.sha256);
        assert_eq!(first_asset.original_name, second_asset.original_name);
        assert_eq!(list_assets(&connection, Some("project")).unwrap().len(), 2);
    }

    #[test]
    fn deduplicates_physical_copy_only_inside_one_project() {
        let directory = tempdir().unwrap();
        let source = source(directory.path(), "master.mov", b"same bytes");
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let first = import_file(&mut connection, &vault, Some("project-a"), &source).unwrap();
        let duplicate = import_file(&mut connection, &vault, Some("project-a"), &source).unwrap();
        let other_project =
            import_file(&mut connection, &vault, Some("project-b"), &source).unwrap();

        assert_ne!(first.id, duplicate.id);
        assert_eq!(first.sha256, duplicate.sha256);
        assert_eq!(
            resolve_original_path(&connection, &vault, &first.id).unwrap(),
            resolve_original_path(&connection, &vault, &duplicate.id).unwrap()
        );
        assert_ne!(
            resolve_original_path(&connection, &vault, &first.id).unwrap(),
            resolve_original_path(&connection, &vault, &other_project.id).unwrap()
        );
        assert_eq!(file_count(&vault), 2);
    }

    #[test]
    fn failed_database_write_removes_staging_and_new_final_file() {
        let directory = tempdir().unwrap();
        let source = source(directory.path(), "failure.bin", b"must be cleaned");
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_asset_insert BEFORE INSERT ON assets
                 BEGIN SELECT RAISE(ABORT, 'forced asset insert failure'); END;",
            )
            .unwrap();

        let error = import_file(&mut connection, &vault, Some("project"), &source).unwrap_err();

        assert_eq!(error.code, "HOST_INTERNAL");
        assert!(list_assets(&connection, None).unwrap().is_empty());
        assert_eq!(file_count(&vault), 0);
        assert_eq!(recovery_intent_count(&vault), 0);
    }

    #[test]
    fn magic_bytes_override_a_misleading_extension() {
        let directory = tempdir().unwrap();
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        png.extend_from_slice(b"minimal fixture");
        let source = source(directory.path(), "misleading.mp4", &png);
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let asset = import_file(&mut connection, &vault, None, &source).unwrap();

        assert_eq!(asset.kind, AssetKind::Image);
        assert_eq!(asset.mime_type, "image/png");
    }

    #[test]
    fn rejects_directories_without_leaving_files_or_records() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let error = import_file(&mut connection, &vault, None, directory.path()).unwrap_err();

        assert_eq!(error.code, "ASSET_SOURCE_INVALID");
        assert!(list_assets(&connection, None).unwrap().is_empty());
        assert_eq!(file_count(&vault), 0);
    }

    #[test]
    fn command_is_idempotent_and_emits_exactly_once() {
        let directory = tempdir().unwrap();
        let source = source(directory.path(), "idempotent.png", b"stable command bytes");
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        let key = "asset-import-idempotent-001";

        let first = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                key,
                "selection-token-001",
                Some("project-a"),
                Some(now_millis() + 30_000),
            ),
            &source,
        )
        .unwrap();
        let replay = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                key,
                "selection-token-001",
                Some("project-a"),
                Some(now_millis() + 60_000),
            ),
            &source,
        )
        .unwrap();

        assert!(!first.response.replayed);
        assert_eq!(first.emitted_events.len(), 1);
        assert!(replay.response.replayed);
        assert!(replay.emitted_events.is_empty());
        assert_eq!(replay.response.asset.id, first.response.asset.id);
        assert_eq!(list_assets(&connection, None).unwrap().len(), 1);
        let events = replay_asset_events(&connection, 0, 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AssetEventType::Imported);
        assert_eq!(events[0].asset.id, first.response.asset.id);
    }

    #[test]
    fn protocol_compatibility_is_bounded_and_1_2_receipts_replay() {
        let directory = tempdir().unwrap();
        let source = source(directory.path(), "legacy.txt", b"legacy receipt bytes");
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        let mut legacy = import_command(
            &Uuid::new_v4().to_string(),
            "asset-import-protocol-1-2",
            "selection-token-protocol-1-2",
            Some("project-a"),
            None,
        );
        let AssetCommandEnvelope::Import {
            protocol_version, ..
        } = &mut legacy;
        *protocol_version = LEGACY_PROTOCOL_VERSION.to_string();

        let committed =
            execute_import_command(&mut connection, &vault, legacy.clone(), &source).unwrap();
        let replayed = execute_import_command(&mut connection, &vault, legacy, &source).unwrap();

        assert!(!committed.response.replayed);
        assert!(replayed.response.replayed);
        assert_eq!(replayed.response.receipt, committed.response.receipt);
        assert_eq!(replayed.response.asset, committed.response.asset);
        assert!(replayed.emitted_events.is_empty());
        let stored_version: String = connection
            .query_row(
                "SELECT protocol_version FROM asset_command_receipts",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_version, LEGACY_PROTOCOL_VERSION);

        for (supported_version, version_label) in [
            (PROTOCOL_1_3_VERSION, "1-3"),
            (PREVIOUS_PROTOCOL_VERSION, "1-4"),
            (PROTOCOL_VERSION, "1-5"),
        ] {
            let mut supported = import_command(
                &Uuid::new_v4().to_string(),
                &format!("asset-import-protocol-{version_label}"),
                &format!("selection-token-protocol-{version_label}"),
                Some("project-a"),
                None,
            );
            let AssetCommandEnvelope::Import {
                protocol_version, ..
            } = &mut supported;
            *protocol_version = supported_version.to_string();
            execute_import_command(&mut connection, &vault, supported, &source).unwrap();
        }

        for unsupported_version in ["1.1", "1.6"] {
            let mut unsupported = import_command(
                &Uuid::new_v4().to_string(),
                &format!("asset-import-protocol-{unsupported_version}"),
                &format!("selection-token-protocol-{unsupported_version}"),
                Some("project-a"),
                None,
            );
            let AssetCommandEnvelope::Import {
                protocol_version, ..
            } = &mut unsupported;
            *protocol_version = unsupported_version.to_string();
            let error =
                execute_import_command(&mut connection, &vault, unsupported, &source).unwrap_err();
            assert_eq!(error.code, "PROTOCOL_VERSION_UNSUPPORTED");
            assert_eq!(
                error.message,
                format!(
                    "expected protocolVersion {LEGACY_PROTOCOL_VERSION}, {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, or {PROTOCOL_VERSION}, received {unsupported_version}"
                )
            );
        }
        assert_eq!(list_assets(&connection, None).unwrap().len(), 4);
        assert_eq!(replay_asset_events(&connection, 0, 100).unwrap().len(), 4);
    }
    #[test]
    fn committed_command_replays_after_deadline_without_source_file() {
        let directory = tempdir().unwrap();
        let source = source(directory.path(), "deadline.pdf", b"%PDF-1.7\ndeadline");
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        let key = "asset-import-deadline-001";
        let first = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                key,
                "selection-token-deadline",
                None,
                Some(now_millis() + 30_000),
            ),
            &source,
        )
        .unwrap();
        fs::remove_file(&source).unwrap();

        let replay = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                key,
                "selection-token-deadline",
                None,
                Some(now_millis() - 1),
            ),
            &source,
        )
        .unwrap();

        assert!(replay.response.replayed);
        assert_eq!(replay.response.asset.id, first.response.asset.id);
        assert!(replay.emitted_events.is_empty());
        assert_eq!(replay_asset_events(&connection, 0, 100).unwrap().len(), 1);
    }

    #[test]
    fn receipt_replay_never_calls_source_resolver() {
        let directory = tempdir().unwrap();
        let source = source(directory.path(), "resolver.txt", b"resolve only once");
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        let key = "asset-import-resolver-001";
        execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                key,
                "one-shot-source-token",
                Some("project-a"),
                None,
            ),
            &source,
        )
        .unwrap();

        let resolver_called = std::cell::Cell::new(false);
        let replay = execute_import_command_with_resolver(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                key,
                "one-shot-source-token",
                Some("project-a"),
                Some(now_millis() - 1),
            ),
            || {
                resolver_called.set(true);
                Err(HostError::new(
                    "SOURCE_TOKEN_ALREADY_CONSUMED",
                    "resolver must not run for receipt replay",
                    false,
                ))
            },
        )
        .unwrap();

        assert!(replay.response.replayed);
        assert!(!resolver_called.get());
        assert!(replay.emitted_events.is_empty());
        assert_eq!(list_assets(&connection, None).unwrap().len(), 1);
    }

    #[test]
    fn command_reuse_errors_are_explicit() {
        let directory = tempdir().unwrap();
        let source = source(directory.path(), "reuse.txt", b"reuse checks");
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        let command_id = Uuid::new_v4().to_string();
        execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &command_id,
                "asset-import-reuse-001",
                "selection-token-original",
                Some("project-a"),
                None,
            ),
            &source,
        )
        .unwrap();

        let key_error = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                "asset-import-reuse-001",
                "selection-token-different",
                Some("project-a"),
                None,
            ),
            &source,
        )
        .unwrap_err();
        assert_eq!(key_error.code, "IDEMPOTENCY_KEY_REUSED");

        let command_error = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &command_id,
                "asset-import-reuse-002",
                "selection-token-original",
                Some("project-a"),
                None,
            ),
            &source,
        )
        .unwrap_err();
        assert_eq!(command_error.code, "COMMAND_ID_REUSED");
        assert_eq!(list_assets(&connection, None).unwrap().len(), 1);
        assert_eq!(replay_asset_events(&connection, 0, 100).unwrap().len(), 1);
    }

    #[test]
    fn command_storage_and_wire_values_never_leak_token_or_paths() {
        let directory = tempdir().unwrap();
        let source = source(directory.path(), "private.mov", b"private path bytes");
        let vault = directory.path().join("vault-private");
        let token = "opaque-selection-token-private-001";
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let outcome = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                "asset-import-private-001",
                token,
                Some("project-private"),
                Some(now_millis() + 30_000),
            ),
            &source,
        )
        .unwrap();

        let source_path = source.to_string_lossy().to_string();
        let vault_path = vault.to_string_lossy().to_string();
        let response_wire = serde_json::to_string(&outcome.response).unwrap();
        let events_wire = serde_json::to_string(&outcome.emitted_events).unwrap();
        for wire in [&response_wire, &events_wire] {
            assert!(!wire.contains(token));
            assert!(!wire.contains(&source_path));
            assert!(!wire.contains(&vault_path));
        }

        let (fingerprint, response_json, protocol_version, deadline_at): (
            String,
            String,
            String,
            Option<i64>,
        ) = connection
            .query_row(
                "SELECT request_fingerprint, response_json, protocol_version, deadline_at
                 FROM asset_command_receipts",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        let event_json: String = connection
            .query_row("SELECT payload_json FROM asset_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(fingerprint.len(), 64);
        assert_eq!(protocol_version, PROTOCOL_VERSION);
        assert!(deadline_at.is_some());
        for stored in [&fingerprint, &response_json, &event_json] {
            assert!(!stored.contains(token));
            assert!(!stored.contains(&source_path));
            assert!(!stored.contains(&vault_path));
        }
    }

    #[test]
    fn command_transaction_failure_cleans_new_file_and_metadata() {
        let directory = tempdir().unwrap();
        let source = source(
            directory.path(),
            "rollback.bin",
            b"rollback all command state",
        );
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_asset_event BEFORE INSERT ON asset_events
                 BEGIN SELECT RAISE(ABORT, 'forced asset event failure'); END;",
            )
            .unwrap();

        let error = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                "asset-import-rollback-001",
                "selection-token-rollback",
                Some("project-a"),
                None,
            ),
            &source,
        )
        .unwrap_err();

        assert_eq!(error.code, "HOST_INTERNAL");
        assert!(list_assets(&connection, None).unwrap().is_empty());
        assert!(replay_asset_events(&connection, 0, 100).unwrap().is_empty());
        let receipt_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM asset_command_receipts", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(receipt_count, 0);
        assert_eq!(file_count(&vault), 0);
        assert_eq!(recovery_intent_count(&vault), 0);
    }

    #[test]
    fn failed_duplicate_command_never_deletes_reused_original() {
        let directory = tempdir().unwrap();
        let source = source(directory.path(), "dedupe.mov", b"shared physical original");
        let vault = directory.path().join("vault");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        let first = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                "asset-import-dedupe-001",
                "selection-token-dedupe-first",
                Some("project-a"),
                None,
            ),
            &source,
        )
        .unwrap();
        let original_path =
            resolve_original_path(&connection, &vault, &first.response.asset.id).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_duplicate_event BEFORE INSERT ON asset_events
                 BEGIN SELECT RAISE(ABORT, 'forced duplicate event failure'); END;",
            )
            .unwrap();

        let error = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                "asset-import-dedupe-002",
                "selection-token-dedupe-second",
                Some("project-a"),
                None,
            ),
            &source,
        )
        .unwrap_err();

        assert_eq!(error.code, "HOST_INTERNAL");
        assert_eq!(list_assets(&connection, None).unwrap().len(), 1);
        assert_eq!(replay_asset_events(&connection, 0, 100).unwrap().len(), 1);
        assert_eq!(file_count(&vault), 1);
        assert_eq!(recovery_intent_count(&vault), 0);
        assert_eq!(
            fs::read(original_path).unwrap(),
            b"shared physical original"
        );
    }

    #[test]
    fn reconciliation_removes_uncommitted_candidate_and_is_idempotent() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let source = source(directory.path(), "orphan.bin", b"orphan candidate");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let (_asset, final_path, mut pending) =
            persist_pending_import(&mut connection, &vault, &source, false);
        assert!(final_path.exists());
        assert_eq!(recovery_intent_count(&vault), 1);
        pending.preserve_for_recovery();

        reconcile_pending_imports(&connection, &vault).unwrap();
        assert!(!final_path.exists());
        assert_eq!(recovery_intent_count(&vault), 0);
        assert_eq!(file_count(&vault), 0);

        reconcile_pending_imports(&connection, &vault).unwrap();
        assert!(!final_path.exists());
        assert_eq!(recovery_intent_count(&vault), 0);
    }

    #[test]
    fn reconciliation_keeps_exactly_committed_candidate_and_clears_intent() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let source = source(directory.path(), "committed.bin", b"committed candidate");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let (asset, final_path, mut pending) =
            persist_pending_import(&mut connection, &vault, &source, true);
        pending.preserve_for_recovery();
        assert_eq!(recovery_intent_count(&vault), 1);

        reconcile_pending_imports(&connection, &vault).unwrap();
        assert!(final_path.exists());
        assert_eq!(get_asset(&connection, &asset.id).unwrap().id, asset.id);
        assert_eq!(recovery_intent_count(&vault), 0);
        assert_eq!(file_count(&vault), 1);
    }

    #[test]
    fn reconciliation_skips_an_active_import_lease() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let source = source(directory.path(), "active.bin", b"active candidate");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let (_asset, final_path, mut pending) =
            persist_pending_import(&mut connection, &vault, &source, false);
        reconcile_pending_imports(&connection, &vault).unwrap();
        assert!(final_path.exists());
        assert_eq!(recovery_intent_count(&vault), 1);

        pending.preserve_for_recovery();
        reconcile_pending_imports(&connection, &vault).unwrap();
        assert!(!final_path.exists());
        assert_eq!(recovery_intent_count(&vault), 0);
    }

    #[test]
    fn invalid_intent_never_targets_external_paths_or_blocks_valid_recovery() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let orphan_source = source(directory.path(), "valid-orphan.bin", b"valid orphan");
        let outside = source(
            directory.path(),
            "outside-sentinel.bin",
            b"outside sentinel",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let (_asset, valid_path, mut pending) =
            persist_pending_import(&mut connection, &vault, &orphan_source, false);
        pending.preserve_for_recovery();
        let recovery_root =
            prepare_import_recovery_root(&prepare_vault_root(&vault).unwrap()).unwrap();
        let invalid_id = Uuid::new_v4().to_string();
        let invalid_path =
            recovery_root.join(format!("{}{}", invalid_id, IMPORT_RECOVERY_INTENT_SUFFIX));
        let invalid = serde_json::json!({
            "version": IMPORT_RECOVERY_INTENT_VERSION,
            "assetId": invalid_id,
            "sha256": "a".repeat(64),
            "extension": "../outside",
            "sizeBytes": 16,
            "absolutePath": outside.to_string_lossy(),
        });
        fs::write(&invalid_path, serde_json::to_vec(&invalid).unwrap()).unwrap();

        let error = reconcile_pending_imports(&connection, &vault).unwrap_err();
        assert_eq!(error.code, "ASSET_RECOVERY_INVALID");
        assert_eq!(fs::read(&outside).unwrap(), b"outside sentinel");
        assert!(invalid_path.exists());
        assert!(!valid_path.exists());
        assert_eq!(recovery_intent_count(&vault), 1);
    }

    #[test]
    fn successful_import_leaves_no_recovery_intent() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let source = source(directory.path(), "success.bin", b"successful import");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        import_file(&mut connection, &vault, None, &source).unwrap();

        assert_eq!(recovery_intent_count(&vault), 0);
        assert!(vault
            .join(IMPORT_RECOVERY_DIRECTORY)
            .join(IMPORT_RECOVERY_LOCK_FILE)
            .is_file());
        assert_eq!(file_count(&vault), 1);
    }

    #[test]
    fn ambiguous_commit_with_different_winner_replays_without_duplicate_event() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let winner_source = source(directory.path(), "winner.bin", b"winning asset");
        let loser_source = source(directory.path(), "loser.bin", b"losing candidate");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let winner = execute_import_command(
            &mut connection,
            &vault,
            import_command(
                &Uuid::new_v4().to_string(),
                "asset-import-ambiguous-winner",
                "selection-token-ambiguous-winner",
                Some("project-recovery"),
                None,
            ),
            &winner_source,
        )
        .unwrap();
        let (candidate, final_path, pending) =
            persist_pending_import(&mut connection, &vault, &loser_source, false);
        let attempted = AssetCommandResponse {
            receipt: winner.response.receipt.clone(),
            asset: candidate,
            replayed: false,
        };
        let mut pending = Some(pending);

        let outcome = settle_ambiguous_import_commit(
            &connection,
            &mut pending,
            &attempted,
            winner.response.clone(),
        )
        .unwrap();

        assert!(outcome.response.replayed);
        assert_eq!(outcome.response.asset.id, winner.response.asset.id);
        assert!(outcome.emitted_events.is_empty());
        assert!(!final_path.exists());
        assert_eq!(recovery_intent_count(&vault), 0);
        assert_eq!(file_count(&vault), 1);
    }

    #[test]
    fn stale_marker_never_deletes_storage_reused_by_another_asset() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let source = source(directory.path(), "shared.bin", b"shared recovery storage");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();

        let original = import_file(
            &mut connection,
            &vault,
            Some("project-shared-recovery"),
            &source,
        )
        .unwrap();
        let original_path = resolve_original_path(&connection, &vault, &original.id).unwrap();
        let extension = original_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap();
        let mut stale_marker = create_pending_asset_file(
            &prepare_vault_root(&vault).unwrap(),
            original_path.clone(),
            &original,
            &extension,
        )
        .unwrap();
        stale_marker.preserve_for_recovery();

        let sibling = import_file(
            &mut connection,
            &vault,
            Some("project-shared-recovery"),
            &source,
        )
        .unwrap();
        assert_eq!(
            resolve_original_path(&connection, &vault, &sibling.id).unwrap(),
            original_path
        );
        connection
            .execute("DELETE FROM assets WHERE id = ?1", [&original.id])
            .unwrap();

        reconcile_pending_imports(&connection, &vault).unwrap();

        assert_eq!(recovery_intent_count(&vault), 0);
        assert!(original_path.exists());
        assert_eq!(
            fs::read(resolve_original_path(&connection, &vault, &sibling.id).unwrap()).unwrap(),
            b"shared recovery storage"
        );
    }
    #[test]
    fn generic_recovery_precedes_business_provenance_cleanup() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        let user_source = source(directory.path(), "user.txt", b"user asset");
        let generated_source = source(directory.path(), "generated.docx", b"generated asset");
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        crate::business_workspace_service::migrate(&connection).unwrap();

        let user_asset = import_file(
            &mut connection,
            &vault,
            Some("project-business-recovery"),
            &user_source,
        )
        .unwrap();
        let generated_asset = import_business_document(
            &mut connection,
            &vault,
            "project-business-recovery",
            &generated_source,
            &Uuid::new_v4().to_string(),
        )
        .unwrap();
        let generated_path =
            resolve_original_path(&connection, &vault, &generated_asset.id).unwrap();
        let extension = generated_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap();
        let mut pending = create_pending_asset_file(
            &prepare_vault_root(&vault).unwrap(),
            generated_path.clone(),
            &generated_asset,
            &extension,
        )
        .unwrap();
        pending.preserve_for_recovery();

        reconcile_pending_imports(&connection, &vault).unwrap();
        assert!(generated_path.exists());
        assert_eq!(recovery_intent_count(&vault), 0);
        assert_eq!(
            crate::business_workspace_service::reconcile_generated_assets(&mut connection, &vault,)
                .unwrap(),
            1
        );
        assert!(!generated_path.exists());
        assert!(get_asset(&connection, &generated_asset.id).is_err());
        assert_eq!(
            get_asset(&connection, &user_asset.id).unwrap().id,
            user_asset.id
        );
        assert!(resolve_original_path(&connection, &vault, &user_asset.id)
            .unwrap()
            .exists());
    }

    #[test]
    fn internal_vault_directories_reject_files_and_reparse_points() {
        let staging_case = tempdir().unwrap();
        let staging_vault = staging_case.path().join("vault");
        fs::create_dir(&staging_vault).unwrap();
        fs::write(staging_vault.join(".staging"), b"not a directory").unwrap();
        let staging_source = source(staging_case.path(), "staging.bin", b"staging bytes");
        let mut staging_connection = Connection::open_in_memory().unwrap();
        migrate(&staging_connection).unwrap();
        let error = import_file(
            &mut staging_connection,
            &staging_vault,
            None,
            &staging_source,
        )
        .unwrap_err();
        assert_eq!(error.code, "VAULT_PATH_INVALID");
        assert!(list_assets(&staging_connection, None).unwrap().is_empty());

        let object_case = tempdir().unwrap();
        let object_vault = object_case.path().join("vault");
        fs::create_dir(&object_vault).unwrap();
        let object_bytes = b"object directory bytes";
        let sha256 = format!("{:x}", Sha256::digest(object_bytes));
        fs::write(object_vault.join(&sha256[..2]), b"not a directory").unwrap();
        let object_source = source(object_case.path(), "object.bin", object_bytes);
        let mut object_connection = Connection::open_in_memory().unwrap();
        migrate(&object_connection).unwrap();
        let error =
            import_file(&mut object_connection, &object_vault, None, &object_source).unwrap_err();
        assert_eq!(error.code, "VAULT_PATH_INVALID");
        assert!(list_assets(&object_connection, None).unwrap().is_empty());
        assert_eq!(recovery_intent_count(&object_vault), 0);

        let link_case = tempdir().unwrap();
        let link_vault = link_case.path().join("vault");
        let outside = link_case.path().join("outside");
        fs::create_dir(&link_vault).unwrap();
        fs::create_dir(&outside).unwrap();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&outside, link_vault.join(".staging")).is_ok();
        #[cfg(windows)]
        let linked =
            std::os::windows::fs::symlink_dir(&outside, link_vault.join(".staging")).is_ok();
        if linked {
            let link_source = source(link_case.path(), "link.bin", b"link bytes");
            let mut link_connection = Connection::open_in_memory().unwrap();
            migrate(&link_connection).unwrap();
            let error =
                import_file(&mut link_connection, &link_vault, None, &link_source).unwrap_err();
            assert_eq!(error.code, "VAULT_PATH_INVALID");
            assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        }
    }
}
