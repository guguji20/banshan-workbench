use crate::codex_host::REQUIRED_CODEX_VERSION;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const EXPECTED_SCHEMA_VERSION: &str = "1.0";
const EXPECTED_BUNDLE_ID: &str = "bsaigc-business";
const INSTALL_STAMP: &str = ".bsaigc-business-install.json";
const ANYBOX_LICENSE: &str = "ANYBOX-MIT.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BusinessSkillInstallReport {
    pub(crate) bundle_version: String,
    pub(crate) skill_count: usize,
    pub(crate) file_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BusinessSkillBundle {
    schema_version: String,
    bundle_id: String,
    version: String,
    minimum_codex_version: String,
    version_authority: String,
    skills: Vec<BusinessSkillEntry>,
    files: Vec<ManagedFile>,
}

#[derive(Debug, Deserialize)]
struct BusinessSkillEntry {
    id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ManagedFile {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallStamp {
    schema_version: String,
    bundle_id: String,
    bundle_version: String,
    managed_files: Vec<String>,
}

struct CommitRecord {
    destination: PathBuf,
    rollback_copy: Option<PathBuf>,
}

/// Installs the product-owned business skills into the isolated Codex home.
///
/// The installer only overwrites paths declared by the signed-in-product bundle
/// manifest and only removes paths recorded by the previous BSAIGC install
/// stamp. Unknown user files are never enumerated or deleted.
pub(crate) fn install_bundled_business_skills(
    data_root: &Path,
    resource_dir: &Path,
) -> Result<BusinessSkillInstallReport, String> {
    let source_root = locate_source_root(resource_dir)?;
    install_from_source(data_root, &source_root, REQUIRED_CODEX_VERSION)
}

fn locate_source_root(resource_dir: &Path) -> Result<PathBuf, String> {
    let development_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("business-skills");
    let candidates = [
        resource_dir.join("resources").join("business-skills"),
        resource_dir.join("business-skills"),
        development_root,
    ];
    for candidate in candidates {
        if candidate.join("bundle.json").is_file() {
            return candidate
                .canonicalize()
                .map_err(|error| format!("resolve business skill resources failed: {error}"));
        }
    }
    Err("bundled business skill resources are missing".to_string())
}

fn install_from_source(
    data_root: &Path,
    source_root: &Path,
    codex_version: &str,
) -> Result<BusinessSkillInstallReport, String> {
    let source_root = source_root
        .canonicalize()
        .map_err(|error| format!("resolve business skill source failed: {error}"))?;
    let bundle_bytes = fs::read(source_root.join("bundle.json"))
        .map_err(|error| format!("read business skill bundle failed: {error}"))?;
    let bundle: BusinessSkillBundle = serde_json::from_slice(&bundle_bytes)
        .map_err(|error| format!("parse business skill bundle failed: {error}"))?;
    validate_bundle(&bundle, codex_version)?;

    fs::create_dir_all(data_root)
        .map_err(|error| format!("create application data root failed: {error}"))?;
    let data_root = data_root
        .canonicalize()
        .map_err(|error| format!("resolve application data root failed: {error}"))?;
    let codex_home = data_root.join("codex-home");
    let skills_root = codex_home.join("skills");
    create_safe_directory(&data_root, &codex_home)?;
    create_safe_directory(&codex_home, &skills_root)?;

    let transaction_id = Uuid::new_v4().to_string();
    let staging_root = codex_home.join(format!(".bsaigc-business-staging-{transaction_id}"));
    let rollback_root = codex_home.join(format!(".bsaigc-business-rollback-{transaction_id}"));
    fs::create_dir(&staging_root)
        .map_err(|error| format!("create business skill staging directory failed: {error}"))?;
    fs::create_dir(&rollback_root)
        .map_err(|error| format!("create business skill rollback directory failed: {error}"))?;

    let result = install_transaction(
        &bundle,
        &source_root,
        &skills_root,
        &staging_root,
        &rollback_root,
    );
    let _ = fs::remove_dir_all(&staging_root);
    let _ = fs::remove_dir_all(&rollback_root);
    result?;

    install_anybox_license(&source_root, &data_root)?;

    Ok(BusinessSkillInstallReport {
        bundle_version: bundle.version,
        skill_count: bundle.skills.len(),
        file_count: bundle.files.len(),
    })
}

fn install_transaction(
    bundle: &BusinessSkillBundle,
    source_root: &Path,
    skills_root: &Path,
    staging_root: &Path,
    rollback_root: &Path,
) -> Result<(), String> {
    for file in &bundle.files {
        let relative = safe_relative_path(&file.path, "bundle file")?;
        let source = source_root.join(&relative);
        validate_source_file(source_root, &source, file)?;
        let staged = staging_root.join(&relative);
        create_safe_parent(staging_root, &staged)?;
        fs::copy(&source, &staged)
            .map_err(|error| format!("stage business skill file {} failed: {error}", file.path))?;
    }

    let stamp_path = skills_root.join(INSTALL_STAMP);
    let previous = read_previous_stamp(&stamp_path)?;
    let new_paths = bundle
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let stale_paths = previous
        .managed_files
        .iter()
        .filter(|path| !new_paths.contains(*path))
        .cloned()
        .collect::<Vec<_>>();

    let mut committed = Vec::new();
    let commit_result = (|| -> Result<(), String> {
        for file in &bundle.files {
            let relative = safe_relative_path(&file.path, "bundle file")?;
            let staged = staging_root.join(&relative);
            let destination = skills_root.join(&relative);
            backup_destination(
                skills_root,
                rollback_root,
                &relative,
                &destination,
                &mut committed,
            )?;
            atomic_copy_replace(&staged, &destination)?;
        }
        for path in stale_paths {
            let relative = safe_relative_path(&path, "previously managed file")?;
            let destination = skills_root.join(&relative);
            if destination.exists() {
                backup_destination(
                    skills_root,
                    rollback_root,
                    &relative,
                    &destination,
                    &mut committed,
                )?;
                fs::remove_file(&destination).map_err(|error| {
                    format!("remove retired business skill file {path} failed: {error}")
                })?;
            }
        }

        let stamp = InstallStamp {
            schema_version: EXPECTED_SCHEMA_VERSION.to_string(),
            bundle_id: EXPECTED_BUNDLE_ID.to_string(),
            bundle_version: bundle.version.clone(),
            managed_files: new_paths.into_iter().collect(),
        };
        let stamp_bytes = serde_json::to_vec_pretty(&stamp)
            .map_err(|error| format!("serialize business skill install stamp failed: {error}"))?;
        atomic_write(&stamp_path, &stamp_bytes)
    })();

    if let Err(error) = commit_result {
        rollback_commits(&committed);
        return Err(error);
    }
    Ok(())
}

fn validate_bundle(bundle: &BusinessSkillBundle, codex_version: &str) -> Result<(), String> {
    if bundle.schema_version != EXPECTED_SCHEMA_VERSION {
        return Err(format!(
            "unsupported business skill bundle schema {}",
            bundle.schema_version
        ));
    }
    if bundle.bundle_id != EXPECTED_BUNDLE_ID {
        return Err(format!(
            "unexpected business skill bundle id {}",
            bundle.bundle_id
        ));
    }
    if bundle.version_authority != "bundle.json" {
        return Err("business skill bundle has no authoritative version source".to_string());
    }
    if compare_semver(codex_version, &bundle.minimum_codex_version)? < 0 {
        return Err(format!(
            "Codex {codex_version} is older than business skill minimum {}",
            bundle.minimum_codex_version
        ));
    }
    if bundle.skills.is_empty() || bundle.files.is_empty() {
        return Err("business skill bundle is empty".to_string());
    }

    let mut skill_ids = BTreeSet::new();
    for skill in &bundle.skills {
        if !is_safe_skill_id(&skill.id) || !skill_ids.insert(skill.id.as_str()) {
            return Err(format!(
                "invalid or duplicate business skill id {}",
                skill.id
            ));
        }
    }

    let mut paths = BTreeSet::new();
    for file in &bundle.files {
        safe_relative_path(&file.path, "bundle file")?;
        if !paths.insert(file.path.as_str()) {
            return Err(format!(
                "duplicate business skill bundle file {}",
                file.path
            ));
        }
        if file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "invalid SHA-256 for business skill file {}",
                file.path
            ));
        }
    }
    Ok(())
}

fn validate_source_file(
    source_root: &Path,
    source: &Path,
    expected: &ManagedFile,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        format!(
            "inspect business skill file {} failed: {error}",
            expected.path
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "business skill file is not a regular file: {}",
            expected.path
        ));
    }
    let canonical = source.canonicalize().map_err(|error| {
        format!(
            "resolve business skill file {} failed: {error}",
            expected.path
        )
    })?;
    if !canonical.starts_with(source_root) {
        return Err(format!(
            "business skill source escaped its bundle: {}",
            expected.path
        ));
    }

    let mut file = File::open(&canonical)
        .map_err(|error| format!("open business skill file {} failed: {error}", expected.path))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            format!("read business skill file {} failed: {error}", expected.path)
        })?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        digest.update(&buffer[..read]);
    }
    if bytes != expected.bytes {
        return Err(format!(
            "business skill file size mismatch for {}: expected {}, got {bytes}",
            expected.path, expected.bytes
        ));
    }
    let actual = format!("{:x}", digest.finalize());
    if actual != expected.sha256 {
        return Err(format!(
            "business skill file hash mismatch for {}",
            expected.path
        ));
    }
    Ok(())
}

fn read_previous_stamp(path: &Path) -> Result<InstallStamp, String> {
    if !path.exists() {
        return Ok(InstallStamp::default());
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("read previous business skill install stamp failed: {error}"))?;
    let stamp: InstallStamp = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse previous business skill install stamp failed: {error}"))?;
    if stamp.schema_version != EXPECTED_SCHEMA_VERSION || stamp.bundle_id != EXPECTED_BUNDLE_ID {
        return Err("previous business skill install stamp is not owned by BSAIGC".to_string());
    }
    for file in &stamp.managed_files {
        safe_relative_path(file, "previously managed file")?;
    }
    Ok(stamp)
}

fn backup_destination(
    skills_root: &Path,
    rollback_root: &Path,
    relative: &Path,
    destination: &Path,
    committed: &mut Vec<CommitRecord>,
) -> Result<(), String> {
    create_safe_parent(skills_root, destination)?;
    if destination.exists() {
        let metadata = fs::symlink_metadata(destination)
            .map_err(|error| format!("inspect installed business skill file failed: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "refusing to replace non-regular managed path {}",
                destination.display()
            ));
        }
        let rollback_copy = rollback_root.join(relative);
        create_safe_parent(rollback_root, &rollback_copy)?;
        fs::copy(destination, &rollback_copy)
            .map_err(|error| format!("backup installed business skill file failed: {error}"))?;
        committed.push(CommitRecord {
            destination: destination.to_path_buf(),
            rollback_copy: Some(rollback_copy),
        });
    } else {
        committed.push(CommitRecord {
            destination: destination.to_path_buf(),
            rollback_copy: None,
        });
    }
    Ok(())
}

fn rollback_commits(committed: &[CommitRecord]) {
    for record in committed.iter().rev() {
        match &record.rollback_copy {
            Some(backup) => {
                let _ = atomic_copy_replace(backup, &record.destination);
            }
            None => {
                let _ = fs::remove_file(&record.destination);
            }
        }
    }
}

fn install_anybox_license(source_root: &Path, data_root: &Path) -> Result<(), String> {
    let resources_root = source_root
        .parent()
        .ok_or("business skill source has no resources parent")?;
    let source = resources_root.join("third-party").join(ANYBOX_LICENSE);
    let metadata = fs::symlink_metadata(&source)
        .map_err(|error| format!("inspect Anybox license resource failed: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Anybox license resource is not a regular file".to_string());
    }
    let destination = data_root.join("third-party").join(ANYBOX_LICENSE);
    create_safe_parent(data_root, &destination)?;
    atomic_copy_replace(&source, &destination)
}

fn atomic_copy_replace(source: &Path, destination: &Path) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| format!("destination has no parent: {}", destination.display()))?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            format!(
                "destination has an invalid file name: {}",
                destination.display()
            )
        })?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let bytes = fs::read(source)
        .map_err(|error| format!("read staged business skill file failed: {error}"))?;
    atomic_write_via_temp(destination, &temporary, &bytes)
}

fn atomic_write(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| format!("destination has no parent: {}", destination.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create destination directory failed: {error}"))?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            format!(
                "destination has an invalid file name: {}",
                destination.display()
            )
        })?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    atomic_write_via_temp(destination, &temporary, bytes)
}

fn atomic_write_via_temp(destination: &Path, temporary: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = File::create(temporary)
        .map_err(|error| format!("create temporary business skill file failed: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("write temporary business skill file failed: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("flush temporary business skill file failed: {error}"))?;
    drop(file);

    let previous = destination.with_file_name(format!(
        ".{}.{}.previous",
        destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("managed"),
        Uuid::new_v4()
    ));
    let had_previous = destination.exists();
    if had_previous {
        fs::rename(destination, &previous)
            .map_err(|error| format!("stage previous business skill file failed: {error}"))?;
    }
    if let Err(error) = fs::rename(temporary, destination) {
        if had_previous {
            let _ = fs::rename(&previous, destination);
        }
        let _ = fs::remove_file(temporary);
        return Err(format!("activate business skill file failed: {error}"));
    }
    if had_previous {
        let _ = fs::remove_file(previous);
    }
    Ok(())
}

fn create_safe_parent(root: &Path, destination: &Path) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| format!("destination has no parent: {}", destination.display()))?;
    create_safe_directory(root, parent)
}

fn create_safe_directory(root: &Path, directory: &Path) -> Result<(), String> {
    let relative = directory
        .strip_prefix(root)
        .map_err(|_| format!("directory escaped managed root: {}", directory.display()))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(format!(
                "invalid managed directory: {}",
                directory.display()
            ));
        };
        current.push(segment);
        if current.exists() {
            let metadata = fs::symlink_metadata(&current)
                .map_err(|error| format!("inspect managed directory failed: {error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "managed directory is not safe: {}",
                    current.display()
                ));
            }
        } else {
            fs::create_dir(&current)
                .map_err(|error| format!("create managed directory failed: {error}"))?;
        }
    }
    Ok(())
}

fn safe_relative_path(value: &str, label: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.contains('\\') || value.starts_with('/') {
        return Err(format!("{label} is not a portable relative path: {value}"));
    }
    let path = Path::new(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("{label} contains an unsafe component: {value}"));
    }
    Ok(path.to_path_buf())
}

fn is_safe_skill_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
}

fn compare_semver(left: &str, right: &str) -> Result<i8, String> {
    let left = parse_semver(left)?;
    let right = parse_semver(right)?;
    for index in 0..3 {
        if left[index] < right[index] {
            return Ok(-1);
        }
        if left[index] > right[index] {
            return Ok(1);
        }
    }
    Ok(0)
}

fn parse_semver(value: &str) -> Result<[u64; 3], String> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(format!("invalid semantic version {value}"));
    }
    let mut parsed = [0_u64; 3];
    for (index, part) in parts.into_iter().enumerate() {
        parsed[index] = part
            .parse::<u64>()
            .map_err(|_| format!("invalid semantic version {value}"))?;
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_production_bundle_without_deleting_unknown_files() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("business-skills");
        let temp = tempfile::tempdir().unwrap();
        let unknown = temp
            .path()
            .join("codex-home")
            .join("skills")
            .join("user-owned")
            .join("SKILL.md");
        fs::create_dir_all(unknown.parent().unwrap()).unwrap();
        fs::write(&unknown, "user skill").unwrap();

        let report = install_from_source(temp.path(), &source, REQUIRED_CODEX_VERSION).unwrap();
        assert_eq!(report.skill_count, 14);
        assert_eq!(report.file_count, 35);
        assert!(temp
            .path()
            .join("codex-home/skills/contract-review/SKILL.md")
            .is_file());
        assert_eq!(fs::read_to_string(unknown).unwrap(), "user skill");
        assert!(temp.path().join("third-party/ANYBOX-MIT.txt").is_file());
    }

    #[test]
    fn rejects_unsafe_bundle_paths() {
        assert!(safe_relative_path("../escape", "test").is_err());
        assert!(safe_relative_path("C:\\escape", "test").is_err());
        assert!(safe_relative_path("ok/skill.md", "test").is_ok());
    }

    #[test]
    fn compares_pinned_semantic_versions() {
        assert_eq!(compare_semver("0.144.5", "0.144.5").unwrap(), 0);
        assert_eq!(compare_semver("0.145.0", "0.144.5").unwrap(), 1);
        assert_eq!(compare_semver("0.143.9", "0.144.5").unwrap(), -1);
    }
}
