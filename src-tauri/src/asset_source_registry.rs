use crate::protocol::{AssetKind, AssetSourceSelection, HostError};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

const DEFAULT_TOKEN_TTL: Duration = Duration::from_secs(10 * 60);
const DEFAULT_MAX_TOKENS: usize = 100;

struct SourceToken {
    path: PathBuf,
    expires_at: Instant,
}

pub struct AssetSourceRegistry {
    tokens: Mutex<HashMap<String, SourceToken>>,
    token_ttl: Duration,
    max_tokens: usize,
}

impl Default for AssetSourceRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_TOKEN_TTL, DEFAULT_MAX_TOKENS)
    }
}

impl AssetSourceRegistry {
    pub fn new(token_ttl: Duration, max_tokens: usize) -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            token_ttl,
            max_tokens: max_tokens.max(1),
        }
    }

    pub fn issue(&self, path: PathBuf) -> Result<AssetSourceSelection, HostError> {
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            HostError::new(
                "ASSET_SOURCE_UNAVAILABLE",
                format!("selected source is unavailable: {error}"),
                false,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(HostError::new(
                "ASSET_SOURCE_INVALID",
                "selected source must be a regular file",
                false,
            ));
        }
        let display_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| HostError::validation("selected source has no valid file name"))?
            .to_string();
        let size_bytes = i64::try_from(metadata.len())
            .map_err(|_| HostError::validation("selected source is too large"))?;
        let source_token = Uuid::new_v4().to_string();

        let mut tokens = self
            .tokens
            .lock()
            .map_err(|_| HostError::internal("asset source token lock is poisoned"))?;
        remove_expired(&mut tokens);
        if tokens.len() >= self.max_tokens {
            if let Some(oldest) = tokens
                .iter()
                .min_by_key(|(_, value)| value.expires_at)
                .map(|(key, _)| key.clone())
            {
                tokens.remove(&oldest);
            }
        }
        tokens.insert(
            source_token.clone(),
            SourceToken {
                path: path.clone(),
                expires_at: Instant::now() + self.token_ttl,
            },
        );
        Ok(AssetSourceSelection {
            source_token,
            display_name,
            detected_kind: preliminary_kind(&path),
            size_bytes,
        })
    }

    pub fn consume(&self, source_token: &str) -> Result<PathBuf, HostError> {
        if Uuid::parse_str(source_token).is_err() {
            return Err(HostError::validation("sourceToken must be a UUID"));
        }
        let mut tokens = self
            .tokens
            .lock()
            .map_err(|_| HostError::internal("asset source token lock is poisoned"))?;
        remove_expired(&mut tokens);
        tokens
            .remove(source_token)
            .map(|entry| entry.path)
            .ok_or_else(|| {
                HostError::new(
                    "ASSET_SOURCE_TOKEN_INVALID",
                    "asset source token is missing, expired, or already used",
                    false,
                )
            })
    }
}

fn remove_expired(tokens: &mut HashMap<String, SourceToken>) {
    let now = Instant::now();
    tokens.retain(|_, value| value.expires_at > now);
}

fn preliminary_kind(path: &Path) -> AssetKind {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "tif" | "tiff" => AssetKind::Image,
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" => AssetKind::Video,
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" => AssetKind::Audio,
        "pdf" | "doc" | "docx" | "ppt" | "pptx" | "xls" | "xlsx" | "txt" | "md" | "json"
        | "csv" => AssetKind::Document,
        _ => AssetKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn token_hides_path_and_is_single_use() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("reference.png");
        fs::write(&path, b"not-a-real-image").unwrap();
        let registry = AssetSourceRegistry::default();

        let selection = registry.issue(path.clone()).unwrap();
        assert_eq!(selection.display_name, "reference.png");
        assert_eq!(selection.detected_kind, AssetKind::Image);
        let serialized = serde_json::to_string(&selection).unwrap();
        assert!(!serialized.contains(directory.path().to_string_lossy().as_ref()));

        assert_eq!(registry.consume(&selection.source_token).unwrap(), path);
        assert_eq!(
            registry.consume(&selection.source_token).unwrap_err().code,
            "ASSET_SOURCE_TOKEN_INVALID"
        );
    }

    #[test]
    fn expired_token_is_rejected() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("clip.mp4");
        fs::write(&path, b"clip").unwrap();
        let registry = AssetSourceRegistry::new(Duration::ZERO, 2);
        let selection = registry.issue(path).unwrap();
        assert_eq!(
            registry.consume(&selection.source_token).unwrap_err().code,
            "ASSET_SOURCE_TOKEN_INVALID"
        );
    }
}
