use crate::protocol::{BrainProjectWorkspaceBinding, BrainWorkspaceSelection, HostError};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_WORKSPACES: usize = 256;

pub struct BrainWorkspaceRegistry {
    workspaces: Mutex<HashMap<String, PathBuf>>,
}

impl Default for BrainWorkspaceRegistry {
    fn default() -> Self {
        Self {
            workspaces: Mutex::new(HashMap::new()),
        }
    }
}

impl BrainWorkspaceRegistry {
    pub fn issue(&self, path: PathBuf) -> Result<BrainWorkspaceSelection, HostError> {
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            HostError::new(
                "BRAIN_WORKSPACE_UNAVAILABLE",
                format!("selected workspace is unavailable: {error}"),
                false,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(HostError::new(
                "BRAIN_WORKSPACE_INVALID",
                "selected workspace must be a regular directory",
                false,
            ));
        }
        let resolved = path.canonicalize().map_err(|error| {
            HostError::new(
                "BRAIN_WORKSPACE_UNAVAILABLE",
                format!("resolve selected workspace failed: {error}"),
                false,
            )
        })?;
        let display_name = resolved
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("工作区")
            .to_string();
        let workspace_token = Uuid::new_v4().to_string();
        let mut workspaces = self
            .workspaces
            .lock()
            .map_err(|_| HostError::internal("brain workspace registry lock is poisoned"))?;
        if workspaces.len() >= MAX_WORKSPACES {
            if let Some(oldest) = workspaces.keys().next().cloned() {
                workspaces.remove(&oldest);
            }
        }
        workspaces.insert(workspace_token.clone(), resolved);
        Ok(BrainWorkspaceSelection {
            workspace_token,
            display_name,
        })
    }

    pub fn resolve(&self, workspace_token: &str) -> Result<PathBuf, HostError> {
        if Uuid::parse_str(workspace_token).is_err() {
            return Err(HostError::validation("workspaceToken must be a UUID"));
        }
        let path = self
            .workspaces
            .lock()
            .map_err(|_| HostError::internal("brain workspace registry lock is poisoned"))?
            .get(workspace_token)
            .cloned()
            .ok_or_else(|| {
                HostError::new(
                    "BRAIN_WORKSPACE_TOKEN_INVALID",
                    "selected workspace is no longer available; choose it again",
                    false,
                )
            })?;
        let metadata = fs::metadata(&path).map_err(|error| {
            HostError::new(
                "BRAIN_WORKSPACE_UNAVAILABLE",
                format!("selected workspace is unavailable: {error}"),
                false,
            )
        })?;
        if !metadata.is_dir() {
            return Err(HostError::new(
                "BRAIN_WORKSPACE_INVALID",
                "selected workspace is no longer a directory",
                false,
            ));
        }
        Ok(path)
    }
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS brain_project_workspaces (
                project_id TEXT PRIMARY KEY,
                canonical_path TEXT NOT NULL,
                path_key TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_brain_project_workspaces_updated
                ON brain_project_workspaces(updated_at DESC, project_id);
            "#,
        )
        .map_err(sql_error)
}

pub fn bind_project(
    connection: &Connection,
    registry: &BrainWorkspaceRegistry,
    project_id: &str,
    workspace_token: &str,
    expected_revision: Option<i64>,
) -> Result<BrainProjectWorkspaceBinding, HostError> {
    validate_project_id(project_id)?;
    let path = registry.resolve(workspace_token)?;
    let canonical_path = path.to_string_lossy().to_string();
    let display_name = workspace_display_name(&path);
    let path_key = normalized_path_key(&canonical_path);
    let project_exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
            params![project_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql_error)?;
    if !project_exists {
        return Err(HostError::new(
            "BRAIN_PROJECT_NOT_FOUND",
            "project does not exist",
            false,
        ));
    }

    let existing = load_stored_binding(connection, project_id)?;
    if let Some(existing) = &existing {
        if existing.path_key == path_key {
            return Ok(BrainProjectWorkspaceBinding {
                project_id: project_id.to_string(),
                workspace_token: workspace_token.to_string(),
                display_name: existing.display_name.clone(),
                revision: existing.revision,
            });
        }
        if expected_revision != Some(existing.revision) {
            return Err(revision_conflict(existing.revision));
        }
    } else if expected_revision.is_some() {
        return Err(revision_conflict(0));
    }

    let now = now_millis()?;
    let next_revision = existing.as_ref().map_or(1, |value| value.revision + 1);
    connection
        .execute(
            "INSERT INTO brain_project_workspaces
             (project_id, canonical_path, path_key, display_name, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(project_id) DO UPDATE SET
               canonical_path = excluded.canonical_path,
               path_key = excluded.path_key,
               display_name = excluded.display_name,
               revision = excluded.revision,
               updated_at = excluded.updated_at",
            params![
                project_id,
                canonical_path,
                path_key,
                display_name,
                next_revision,
                now
            ],
        )
        .map_err(|error| {
            if error
                .to_string()
                .contains("brain_project_workspaces.path_key")
            {
                HostError::new(
                    "BRAIN_WORKSPACE_ALREADY_BOUND",
                    "selected workspace is already bound to another project",
                    false,
                )
            } else {
                sql_error(error)
            }
        })?;
    Ok(BrainProjectWorkspaceBinding {
        project_id: project_id.to_string(),
        workspace_token: workspace_token.to_string(),
        display_name,
        revision: next_revision,
    })
}

pub fn list_projects(
    connection: &Connection,
    registry: &BrainWorkspaceRegistry,
) -> Result<Vec<BrainProjectWorkspaceBinding>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT project_id, canonical_path, display_name, revision
             FROM brain_project_workspaces
             ORDER BY updated_at DESC, project_id ASC",
        )
        .map_err(sql_error)?;
    let stored = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    let mut bindings = Vec::with_capacity(stored.len());
    for (project_id, path, display_name, revision) in stored {
        match registry.issue(PathBuf::from(path)) {
            Ok(selection) => bindings.push(BrainProjectWorkspaceBinding {
                project_id,
                workspace_token: selection.workspace_token,
                display_name,
                revision,
            }),
            Err(error) => eprintln!(
                "brain project workspace reissue deferred: project_id={} code={}",
                project_id, error.code
            ),
        }
    }
    Ok(bindings)
}

pub fn unbind_project(
    connection: &Connection,
    project_id: &str,
    expected_revision: i64,
) -> Result<(), HostError> {
    validate_project_id(project_id)?;
    let existing = load_stored_binding(connection, project_id)?.ok_or_else(|| {
        HostError::new(
            "BRAIN_PROJECT_WORKSPACE_NOT_FOUND",
            "project workspace binding does not exist",
            false,
        )
    })?;
    if existing.revision != expected_revision {
        return Err(revision_conflict(existing.revision));
    }
    connection
        .execute(
            "DELETE FROM brain_project_workspaces WHERE project_id = ?1 AND revision = ?2",
            params![project_id, expected_revision],
        )
        .map_err(sql_error)?;
    Ok(())
}

#[derive(Debug)]
struct StoredBinding {
    path_key: String,
    display_name: String,
    revision: i64,
}

fn load_stored_binding(
    connection: &Connection,
    project_id: &str,
) -> Result<Option<StoredBinding>, HostError> {
    connection
        .query_row(
            "SELECT path_key, display_name, revision
             FROM brain_project_workspaces WHERE project_id = ?1",
            params![project_id],
            |row| {
                Ok(StoredBinding {
                    path_key: row.get(0)?,
                    display_name: row.get(1)?,
                    revision: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(sql_error)
}

fn workspace_display_name(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("工作区")
        .to_string()
}

fn normalized_path_key(path: &str) -> String {
    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path.to_string()
    }
}

fn validate_project_id(project_id: &str) -> Result<(), HostError> {
    Uuid::parse_str(project_id)
        .map(|_| ())
        .map_err(|_| HostError::validation("projectId must be a UUID"))
}

fn now_millis() -> Result<i64, HostError> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HostError::internal("system clock is before the Unix epoch"))?
        .as_millis();
    i64::try_from(value).map_err(|_| HostError::internal("system clock exceeds i64"))
}

fn revision_conflict(actual_revision: i64) -> HostError {
    HostError::new(
        "BRAIN_PROJECT_WORKSPACE_REVISION_CONFLICT",
        format!("project workspace revision conflict; actualRevision={actual_revision}"),
        true,
    )
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::internal(format!("brain project workspace SQLite failure: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn database_with_project() -> (Connection, String) {
        let database = Connection::open_in_memory().unwrap();
        database
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE projects (id TEXT PRIMARY KEY);",
            )
            .unwrap();
        let project_id = Uuid::new_v4().to_string();
        database
            .execute("INSERT INTO projects (id) VALUES (?1)", params![project_id])
            .unwrap();
        migrate(&database).unwrap();
        (database, project_id)
    }

    fn assert_binding_dto_hides_path(binding: &BrainProjectWorkspaceBinding, canonical_path: &str) {
        let value = serde_json::to_value(binding).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 4);
        for field in ["projectId", "workspaceToken", "displayName", "revision"] {
            assert!(object.contains_key(field), "missing DTO field: {field}");
        }
        assert!(object
            .values()
            .all(|value| value.as_str() != Some(canonical_path)));
    }

    #[test]
    fn selection_hides_path_and_resolves_only_registered_directory() {
        let directory = tempdir().unwrap();
        let workspace = directory.path().join("customer-project");
        fs::create_dir(&workspace).unwrap();
        let registry = BrainWorkspaceRegistry::default();

        let selection = registry.issue(workspace.clone()).unwrap();
        assert_eq!(selection.display_name, "customer-project");
        assert!(!serde_json::to_string(&selection)
            .unwrap()
            .contains(directory.path().to_string_lossy().as_ref()));
        assert_eq!(
            registry.resolve(&selection.workspace_token).unwrap(),
            workspace.canonicalize().unwrap()
        );
    }

    #[test]
    fn bound_project_is_reissued_with_a_fresh_token_after_registry_restart() {
        let directory = tempdir().unwrap();
        let workspace = directory.path().join("customer-project");
        fs::create_dir(&workspace).unwrap();
        let (database, project_id) = database_with_project();
        let first_registry = BrainWorkspaceRegistry::default();
        let selected = first_registry.issue(workspace.clone()).unwrap();
        let binding = bind_project(
            &database,
            &first_registry,
            &project_id,
            &selected.workspace_token,
            None,
        )
        .unwrap();
        assert_eq!(binding.revision, 1);

        let restarted_registry = BrainWorkspaceRegistry::default();
        let restored = list_projects(&database, &restarted_registry).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].project_id, project_id);
        assert_ne!(restored[0].workspace_token, selected.workspace_token);
        assert_eq!(
            restarted_registry
                .resolve(&restored[0].workspace_token)
                .unwrap(),
            workspace.canonicalize().unwrap()
        );
    }

    #[test]
    fn serialized_binding_dto_never_contains_the_absolute_canonical_path() {
        let directory = tempdir().unwrap();
        let workspace = directory.path().join("customer-project");
        fs::create_dir(&workspace).unwrap();
        let canonical_path = workspace.canonicalize().unwrap();
        let canonical_path = canonical_path.to_string_lossy();
        let (database, project_id) = database_with_project();
        let registry = BrainWorkspaceRegistry::default();
        let selected = registry.issue(workspace).unwrap();
        let binding = bind_project(
            &database,
            &registry,
            &project_id,
            &selected.workspace_token,
            None,
        )
        .unwrap();
        assert_binding_dto_hides_path(&binding, canonical_path.as_ref());

        let restarted_registry = BrainWorkspaceRegistry::default();
        let restored = list_projects(&database, &restarted_registry).unwrap();
        assert_eq!(restored.len(), 1);
        assert_binding_dto_hides_path(&restored[0], canonical_path.as_ref());
    }

    #[test]
    fn bind_rejects_a_stale_expected_revision() {
        let directory = tempdir().unwrap();
        let first_workspace = directory.path().join("first-workspace");
        let second_workspace = directory.path().join("second-workspace");
        fs::create_dir(&first_workspace).unwrap();
        fs::create_dir(&second_workspace).unwrap();
        let (database, project_id) = database_with_project();
        let registry = BrainWorkspaceRegistry::default();
        let first_selection = registry.issue(first_workspace.clone()).unwrap();
        let binding = bind_project(
            &database,
            &registry,
            &project_id,
            &first_selection.workspace_token,
            None,
        )
        .unwrap();
        let second_selection = registry.issue(second_workspace).unwrap();

        let error = bind_project(
            &database,
            &registry,
            &project_id,
            &second_selection.workspace_token,
            Some(binding.revision - 1),
        )
        .unwrap_err();
        assert_eq!(error.code, "BRAIN_PROJECT_WORKSPACE_REVISION_CONFLICT");
        assert!(error.message.contains("actualRevision=1"));
        assert!(error.retryable);

        let restarted_registry = BrainWorkspaceRegistry::default();
        let restored = list_projects(&database, &restarted_registry).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].revision, binding.revision);
        assert_eq!(
            restarted_registry
                .resolve(&restored[0].workspace_token)
                .unwrap(),
            first_workspace.canonicalize().unwrap()
        );
    }

    #[test]
    fn unbound_project_stays_absent_after_registry_restart() {
        let directory = tempdir().unwrap();
        let workspace = directory.path().join("customer-project");
        fs::create_dir(&workspace).unwrap();
        let (database, project_id) = database_with_project();
        let registry = BrainWorkspaceRegistry::default();
        let selected = registry.issue(workspace).unwrap();
        let binding = bind_project(
            &database,
            &registry,
            &project_id,
            &selected.workspace_token,
            None,
        )
        .unwrap();

        unbind_project(&database, &project_id, binding.revision).unwrap();
        let restarted_registry = BrainWorkspaceRegistry::default();
        assert!(list_projects(&database, &restarted_registry)
            .unwrap()
            .is_empty());
    }
}
