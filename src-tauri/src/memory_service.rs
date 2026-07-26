use crate::protocol::{HostError, MemoryRecord, MemoryScope};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_PAGE_LIMIT: i64 = 100;
const MAX_PAGE_LIMIT: i64 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryQuery {
    pub scope: Option<MemoryScope>,
    pub project_id: Option<String>,
    pub thread_id: Option<String>,
    pub offset: i64,
    pub limit: i64,
}

impl Default for MemoryQuery {
    fn default() -> Self {
        Self {
            scope: None,
            project_id: None,
            thread_id: None,
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
        }
    }
}

#[derive(Serialize)]
struct StoredContentRef<'a> {
    content: &'a str,
    metadata: &'a Value,
}

#[derive(Deserialize)]
struct StoredContent {
    content: String,
    metadata: Value,
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memory_records (
                id TEXT PRIMARY KEY NOT NULL,
                scope TEXT NOT NULL CHECK (scope IN ('global', 'project', 'thread')),
                project_id TEXT,
                thread_id TEXT,
                key TEXT NOT NULL CHECK (length(trim(key)) > 0),
                content_json TEXT NOT NULL CHECK (json_valid(content_json)),
                revision INTEGER NOT NULL CHECK (revision >= 1),
                created_at INTEGER NOT NULL CHECK (created_at > 0),
                updated_at INTEGER NOT NULL CHECK (updated_at >= created_at),
                CHECK (
                    (scope = 'global' AND project_id IS NULL AND thread_id IS NULL)
                    OR (scope = 'project' AND project_id IS NOT NULL AND length(trim(project_id)) > 0 AND thread_id IS NULL)
                    OR (scope = 'thread' AND project_id IS NULL AND thread_id IS NOT NULL AND length(trim(thread_id)) > 0)
                )
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_records_owner_key
                ON memory_records(scope, COALESCE(project_id, ''), COALESCE(thread_id, ''), key);
            CREATE INDEX IF NOT EXISTS idx_memory_records_project
                ON memory_records(project_id, updated_at DESC, id ASC);
            CREATE INDEX IF NOT EXISTS idx_memory_records_thread
                ON memory_records(thread_id, updated_at DESC, id ASC);
            CREATE INDEX IF NOT EXISTS idx_memory_records_scope
                ON memory_records(scope, updated_at DESC, id ASC);
            "#,
        )
        .map_err(sql_error)
}

/// Inserts or replaces the mutable fields of a memory record.
///
/// `revision`, `created_at`, and `updated_at` on the input are ignored because
/// they are service-owned. A new record rejects `expected_revision`; an update
/// requires it and advances the persisted revision by exactly one.
pub fn upsert(
    connection: &Connection,
    record: &MemoryRecord,
    expected_revision: Option<i64>,
) -> Result<MemoryRecord, HostError> {
    let owner = validate_record(record)?;
    let content_json = serialize_content(record)?;
    let current = load_optional(connection, &record.id)?;

    match current {
        None => {
            if expected_revision.is_some() {
                return Err(HostError::validation(
                    "new memory records reject expectedRevision",
                ));
            }
            ensure_unique_key(connection, &record.id, record, &owner)?;
            let now = now_millis().max(1);
            connection
                .execute(
                    "INSERT INTO memory_records
                     (id, scope, project_id, thread_id, key, content_json, revision, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?7)",
                    params![
                        record.id,
                        scope_to_db(&record.scope),
                        owner.project_id,
                        owner.thread_id,
                        record.memory_type,
                        content_json,
                        now,
                    ],
                )
                .map_err(write_error)?;
        }
        Some(current) => {
            let expected = expected_revision
                .ok_or_else(|| HostError::validation("memory updates require expectedRevision"))?;
            ensure_revision(&current, expected)?;
            ensure_unique_key(connection, &record.id, record, &owner)?;
            let updated_at = now_millis()
                .max(current.updated_at.saturating_add(1))
                .max(current.created_at);
            let changed = connection
                .execute(
                    "UPDATE memory_records
                     SET scope = ?2, project_id = ?3, thread_id = ?4, key = ?5,
                         content_json = ?6, revision = revision + 1, updated_at = ?7
                     WHERE id = ?1 AND revision = ?8",
                    params![
                        record.id,
                        scope_to_db(&record.scope),
                        owner.project_id,
                        owner.thread_id,
                        record.memory_type,
                        content_json,
                        updated_at,
                        expected,
                    ],
                )
                .map_err(write_error)?;
            if changed != 1 {
                return Err(revision_conflict(
                    &record.id,
                    "changed while it was being updated",
                ));
            }
        }
    }

    get(connection, &record.id)
}

pub fn get(connection: &Connection, id: &str) -> Result<MemoryRecord, HostError> {
    validate_id(id)?;
    load_optional(connection, id)?.ok_or_else(|| memory_not_found(id))
}

pub fn list(connection: &Connection, query: &MemoryQuery) -> Result<Vec<MemoryRecord>, HostError> {
    validate_query(query)?;
    query_records(connection, query, None)
}

pub fn search(
    connection: &Connection,
    text: &str,
    query: &MemoryQuery,
) -> Result<Vec<MemoryRecord>, HostError> {
    validate_query(query)?;
    let text = text.trim();
    if text.is_empty() {
        return Err(HostError::validation("memory search text is required"));
    }
    if text.chars().count() > 1_000 {
        return Err(HostError::validation(
            "memory search text exceeds 1000 characters",
        ));
    }
    let pattern = format!("%{}%", escape_like(text));
    query_records(connection, query, Some(&pattern))
}

pub fn delete(
    connection: &Connection,
    id: &str,
    expected_revision: i64,
) -> Result<MemoryRecord, HostError> {
    validate_id(id)?;
    if expected_revision < 1 {
        return Err(HostError::validation(
            "memory delete requires expectedRevision > 0",
        ));
    }
    let current = get(connection, id)?;
    ensure_revision(&current, expected_revision)?;
    let changed = connection
        .execute(
            "DELETE FROM memory_records WHERE id = ?1 AND revision = ?2",
            params![id, expected_revision],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(revision_conflict(id, "changed while it was being deleted"));
    }
    Ok(current)
}

struct Owner<'a> {
    project_id: Option<&'a str>,
    thread_id: Option<&'a str>,
}

fn validate_record(record: &MemoryRecord) -> Result<Owner<'_>, HostError> {
    validate_id(&record.id)?;
    validate_key(&record.memory_type)?;
    match &record.scope {
        MemoryScope::Global => {
            if record.scope_id.is_some() {
                return Err(HostError::validation(
                    "global memory must not declare scopeId",
                ));
            }
            Ok(Owner {
                project_id: None,
                thread_id: None,
            })
        }
        MemoryScope::Project => {
            let project_id = required_scope_id(record, "project")?;
            Ok(Owner {
                project_id: Some(project_id),
                thread_id: None,
            })
        }
        MemoryScope::Thread => {
            let thread_id = required_scope_id(record, "thread")?;
            Ok(Owner {
                project_id: None,
                thread_id: Some(thread_id),
            })
        }
    }
}

fn required_scope_id<'a>(record: &'a MemoryRecord, scope_name: &str) -> Result<&'a str, HostError> {
    let scope_id = record
        .scope_id
        .as_deref()
        .ok_or_else(|| HostError::validation(format!("{scope_name} memory requires scopeId")))?;
    if scope_id.trim().is_empty() || scope_id != scope_id.trim() {
        return Err(HostError::validation(format!(
            "{scope_name} memory scopeId must be non-empty and trimmed"
        )));
    }
    if scope_id.chars().count() > 256 {
        return Err(HostError::validation(format!(
            "{scope_name} memory scopeId exceeds 256 characters"
        )));
    }
    Ok(scope_id)
}

fn validate_id(id: &str) -> Result<(), HostError> {
    if id.trim().is_empty() || id != id.trim() {
        return Err(HostError::validation(
            "memory id must be non-empty and trimmed",
        ));
    }
    if id.chars().count() > 256 {
        return Err(HostError::validation("memory id exceeds 256 characters"));
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), HostError> {
    if key.trim().is_empty() || key != key.trim() {
        return Err(HostError::validation(
            "memory key must be non-empty and trimmed",
        ));
    }
    if key.chars().count() > 256 {
        return Err(HostError::validation("memory key exceeds 256 characters"));
    }
    Ok(())
}

fn validate_query(query: &MemoryQuery) -> Result<(), HostError> {
    if !(1..=MAX_PAGE_LIMIT).contains(&query.limit) {
        return Err(HostError::validation(
            "memory page limit must be between 1 and 500",
        ));
    }
    if query.offset < 0 {
        return Err(HostError::validation(
            "memory page offset must not be negative",
        ));
    }
    for (name, value) in [
        ("projectId", query.project_id.as_deref()),
        ("threadId", query.thread_id.as_deref()),
    ] {
        if let Some(value) = value {
            if value.trim().is_empty() || value != value.trim() {
                return Err(HostError::validation(format!(
                    "memory filter {name} must be non-empty and trimmed"
                )));
            }
        }
    }
    Ok(())
}

fn ensure_unique_key(
    connection: &Connection,
    id: &str,
    record: &MemoryRecord,
    owner: &Owner<'_>,
) -> Result<(), HostError> {
    let conflicting_id = connection
        .query_row(
            "SELECT id FROM memory_records
             WHERE id <> ?1 AND scope = ?2 AND project_id IS ?3 AND thread_id IS ?4 AND key = ?5
             LIMIT 1",
            params![
                id,
                scope_to_db(&record.scope),
                owner.project_id,
                owner.thread_id,
                record.memory_type,
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sql_error)?;
    if conflicting_id.is_some() {
        return Err(memory_key_conflict(&record.memory_type));
    }
    Ok(())
}

fn query_records(
    connection: &Connection,
    query: &MemoryQuery,
    search_pattern: Option<&str>,
) -> Result<Vec<MemoryRecord>, HostError> {
    let scope = query.scope.as_ref().map(scope_to_db);
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, scope, project_id, thread_id, key, content_json,
                   revision, created_at, updated_at
            FROM memory_records
            WHERE (?1 IS NULL OR scope = ?1)
              AND (?2 IS NULL OR project_id = ?2)
              AND (?3 IS NULL OR thread_id = ?3)
              AND (
                  ?4 IS NULL
                  OR key LIKE ?4 ESCAPE '\'
                  OR content_json LIKE ?4 ESCAPE '\'
              )
            ORDER BY updated_at DESC, id ASC
            LIMIT ?5 OFFSET ?6
            "#,
        )
        .map_err(sql_error)?;
    let records = statement
        .query_map(
            params![
                scope,
                query.project_id,
                query.thread_id,
                search_pattern,
                query.limit,
                query.offset,
            ],
            memory_from_row,
        )
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

fn load_optional(connection: &Connection, id: &str) -> Result<Option<MemoryRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, scope, project_id, thread_id, key, content_json,
                    revision, created_at, updated_at
             FROM memory_records WHERE id = ?1",
            [id],
            memory_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn memory_from_row(row: &Row<'_>) -> rusqlite::Result<MemoryRecord> {
    let raw_scope: String = row.get(1)?;
    let scope = scope_from_db(&raw_scope).ok_or_else(|| {
        conversion(
            1,
            format!("unknown memory scope persisted in SQLite: {raw_scope}"),
        )
    })?;
    let project_id: Option<String> = row.get(2)?;
    let thread_id: Option<String> = row.get(3)?;
    let scope_id = match &scope {
        MemoryScope::Global => None,
        MemoryScope::Project => project_id,
        MemoryScope::Thread => thread_id,
    };
    let content_json: String = row.get(5)?;
    let stored: StoredContent = serde_json::from_str(&content_json)
        .map_err(|error| conversion(5, format!("invalid memory content JSON: {error}")))?;
    Ok(MemoryRecord {
        id: row.get(0)?,
        scope,
        scope_id,
        memory_type: row.get(4)?,
        content: stored.content,
        metadata: stored.metadata,
        revision: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn serialize_content(record: &MemoryRecord) -> Result<String, HostError> {
    serde_json::to_string(&StoredContentRef {
        content: &record.content,
        metadata: &record.metadata,
    })
    .map_err(|error| HostError::internal(format!("memory JSON serialization failed: {error}")))
}

fn ensure_revision(record: &MemoryRecord, expected_revision: i64) -> Result<(), HostError> {
    if expected_revision < 1 {
        return Err(HostError::validation(
            "memory expectedRevision must be greater than zero",
        ));
    }
    if record.revision != expected_revision {
        return Err(revision_conflict(
            &record.id,
            &format!(
                "revision is {}, request expected {}",
                record.revision, expected_revision
            ),
        ));
    }
    Ok(())
}

fn scope_to_db(scope: &MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Global => "global",
        MemoryScope::Project => "project",
        MemoryScope::Thread => "thread",
    }
}

fn scope_from_db(value: &str) -> Option<MemoryScope> {
    Some(match value {
        "global" => MemoryScope::Global,
        "project" => MemoryScope::Project,
        "thread" => MemoryScope::Thread,
        _ => return None,
    })
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn memory_not_found(id: &str) -> HostError {
    HostError::new(
        "MEMORY_NOT_FOUND",
        format!("memory record {id} does not exist"),
        false,
    )
}

fn memory_key_conflict(key: &str) -> HostError {
    HostError::new(
        "MEMORY_KEY_CONFLICT",
        format!("memory key {key} already exists in this scope"),
        false,
    )
}

fn revision_conflict(id: &str, detail: &str) -> HostError {
    HostError::conflict(format!("memory record {id} {detail}"))
}

fn conversion(column: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn is_constraint(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

fn write_error(error: rusqlite::Error) -> HostError {
    if is_constraint(&error) {
        HostError::new(
            "MEMORY_KEY_CONFLICT",
            "memory record conflicts with an existing owner/key or violates scope ownership",
            false,
        )
    } else {
        sql_error(error)
    }
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::internal(format!("memory SQLite operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        connection
    }

    fn memory(
        id: &str,
        scope: MemoryScope,
        scope_id: Option<&str>,
        key: &str,
        content: &str,
    ) -> MemoryRecord {
        MemoryRecord {
            id: id.to_string(),
            scope,
            scope_id: scope_id.map(str::to_string),
            memory_type: key.to_string(),
            content: content.to_string(),
            metadata: json!({}),
            revision: 0,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn first_upsert_and_revision_checked_update_are_authoritative() {
        let connection = database();
        let mut candidate = memory(
            "memory-1",
            MemoryScope::Project,
            Some("project-1"),
            "creative.brief",
            "first",
        );
        let created = upsert(&connection, &candidate, None).unwrap();
        assert_eq!(created.revision, 1);
        assert!(created.created_at > 0);
        assert_eq!(created.created_at, created.updated_at);

        candidate.content = "updated".to_string();
        let stale = upsert(&connection, &candidate, Some(2)).unwrap_err();
        assert_eq!(stale.code, "REVISION_CONFLICT");
        assert_eq!(get(&connection, "memory-1").unwrap().content, "first");

        let updated = upsert(&connection, &candidate, Some(1)).unwrap();
        assert_eq!(updated.revision, 2);
        assert_eq!(updated.content, "updated");
        assert!(updated.updated_at > updated.created_at);
        assert_eq!(updated.created_at, created.created_at);

        let missing_revision = upsert(&connection, &candidate, None).unwrap_err();
        assert_eq!(missing_revision.code, "VALIDATION_FAILED");
    }

    #[test]
    fn scope_ownership_is_validated_and_mapped_to_columns() {
        let connection = database();
        let invalid_global = memory(
            "global-invalid",
            MemoryScope::Global,
            Some("unexpected"),
            "key",
            "content",
        );
        assert_eq!(
            upsert(&connection, &invalid_global, None).unwrap_err().code,
            "VALIDATION_FAILED"
        );

        let project_without_owner = memory(
            "project-invalid",
            MemoryScope::Project,
            None,
            "key",
            "content",
        );
        assert_eq!(
            upsert(&connection, &project_without_owner, None)
                .unwrap_err()
                .code,
            "VALIDATION_FAILED"
        );

        let thread = memory(
            "thread-valid",
            MemoryScope::Thread,
            Some("thread-1"),
            "summary",
            "content",
        );
        upsert(&connection, &thread, None).unwrap();
        let columns: (Option<String>, Option<String>) = connection
            .query_row(
                "SELECT project_id, thread_id FROM memory_records WHERE id = ?1",
                ["thread-valid"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(columns, (None, Some("thread-1".to_string())));
    }

    #[test]
    fn key_is_unique_inside_each_scope_owner() {
        let connection = database();
        upsert(
            &connection,
            &memory(
                "one",
                MemoryScope::Project,
                Some("project-1"),
                "brief",
                "one",
            ),
            None,
        )
        .unwrap();
        let duplicate = upsert(
            &connection,
            &memory(
                "two",
                MemoryScope::Project,
                Some("project-1"),
                "brief",
                "two",
            ),
            None,
        )
        .unwrap_err();
        assert_eq!(duplicate.code, "MEMORY_KEY_CONFLICT");

        upsert(
            &connection,
            &memory(
                "three",
                MemoryScope::Project,
                Some("project-2"),
                "brief",
                "three",
            ),
            None,
        )
        .unwrap();
        upsert(
            &connection,
            &memory("global", MemoryScope::Global, None, "brief", "global"),
            None,
        )
        .unwrap();
        assert_eq!(list(&connection, &MemoryQuery::default()).unwrap().len(), 3);
    }

    #[test]
    fn list_filters_pages_and_searches_only_key_and_content_json() {
        let connection = database();
        let mut literal = memory(
            "literal",
            MemoryScope::Project,
            Some("project-1"),
            "rate%_card",
            "pricing",
        );
        literal.metadata = json!({"tag": "quoted needle"});
        upsert(&connection, &literal, None).unwrap();
        upsert(
            &connection,
            &memory(
                "owner-only-token",
                MemoryScope::Project,
                Some("project-token"),
                "music",
                "ambient",
            ),
            None,
        )
        .unwrap();
        upsert(
            &connection,
            &memory(
                "other",
                MemoryScope::Thread,
                Some("thread-1"),
                "summary",
                "interview notes",
            ),
            None,
        )
        .unwrap();

        let project_query = MemoryQuery {
            scope: Some(MemoryScope::Project),
            project_id: Some("project-1".to_string()),
            limit: 10,
            ..MemoryQuery::default()
        };
        assert_eq!(list(&connection, &project_query).unwrap().len(), 1);

        let one_per_page = MemoryQuery {
            limit: 1,
            ..MemoryQuery::default()
        };
        let first = list(&connection, &one_per_page).unwrap();
        let second = list(
            &connection,
            &MemoryQuery {
                offset: 1,
                ..one_per_page
            },
        )
        .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_ne!(first[0].id, second[0].id);

        assert_eq!(
            search(&connection, "needle", &MemoryQuery::default())
                .unwrap()
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            vec!["literal"]
        );
        assert_eq!(
            search(&connection, "%_", &MemoryQuery::default())
                .unwrap()
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            vec!["literal"]
        );
        assert!(search(&connection, "token", &project_query)
            .unwrap()
            .is_empty());

        let invalid_page = MemoryQuery {
            limit: 501,
            ..MemoryQuery::default()
        };
        assert_eq!(
            list(&connection, &invalid_page).unwrap_err().code,
            "VALIDATION_FAILED"
        );
    }

    #[test]
    fn delete_requires_matching_revision() {
        let connection = database();
        let created = upsert(
            &connection,
            &memory("delete-me", MemoryScope::Global, None, "key", "content"),
            None,
        )
        .unwrap();

        let stale = delete(&connection, &created.id, created.revision + 1).unwrap_err();
        assert_eq!(stale.code, "REVISION_CONFLICT");
        assert!(get(&connection, &created.id).is_ok());

        let deleted = delete(&connection, &created.id, created.revision).unwrap();
        assert_eq!(deleted, created);
        assert_eq!(
            get(&connection, &created.id).unwrap_err().code,
            "MEMORY_NOT_FOUND"
        );
    }
}
