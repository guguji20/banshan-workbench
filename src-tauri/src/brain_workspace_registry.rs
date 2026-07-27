use crate::protocol::{BrainWorkspaceSelection, HostError};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

const MAX_WORKSPACES: usize = 32;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
}
