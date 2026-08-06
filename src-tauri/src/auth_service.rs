//! Local login with an optional shared account registry on Cloudflare R2.
//!
//! Design contract:
//! - The local SQLite table is authoritative for THIS machine and keeps
//!   working offline. Every mutation lands locally first.
//! - When the `BSAIGC_R2_*` backup configuration is present, the account
//!   table is mirrored as one small JSON object in the bucket so every
//!   machine of the company shares the same accounts (last-writer-wins by
//!   monotonically increasing revision; sufficient for a small office).
//! - Passwords are stored as PBKDF2-HMAC-SHA256 hashes (per-user random
//!   salt, iteration count stored per record). No plaintext, no new crates.
//! - This is an office-level access gate, not disk encryption: someone with
//!   the database file can read business data. Documented in the release
//!   notes; encryption-at-rest would be a separate project.

use crate::protocol::{
    AppUserRecord, AppUserRole, AppUserStatus, AuthChangePasswordPayload, AuthCreateUserPayload,
    AuthCredentials, AuthDeleteUserPayload, AuthRegistrySync, AuthResetPasswordPayload, AuthStatus,
    AuthUsersSnapshot, HostError,
};
use crate::r2_backup::{RegistryStore, RegistryStoreLoad};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const REGISTRY_OBJECT: &str = "auth-users.json";
const REGISTRY_VERSION: u32 = 1;
const PBKDF2_ITERATIONS: u32 = 60_000;
const MIN_PASSWORD_CHARS: usize = 6;
const MAX_PASSWORD_CHARS: usize = 128;
const MAX_USERNAME_CHARS: usize = 32;
const MAX_USERS: usize = 500;

const META_REGISTRY_REVISION: &str = "registry_revision";
const META_REGISTRY_DIRTY: &str = "registry_dirty";

pub struct AuthService {
    connection: Arc<Mutex<Connection>>,
    registry: Option<RegistryStore>,
    registry_state: Mutex<RegistryFeedback>,
    session: Mutex<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthPrincipal {
    pub(crate) username: String,
    pub(crate) role: AppUserRole,
}

#[derive(Clone)]
struct RegistryFeedback {
    sync: AuthRegistrySync,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryDocument {
    version: u32,
    revision: i64,
    updated_at: i64,
    users: Vec<RegistryUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryUser {
    username: String,
    role: String,
    status: String,
    salt_hex: String,
    hash_hex: String,
    iterations: u32,
    updated_at: i64,
}

struct StoredUser {
    username: String,
    role: AppUserRole,
    status: AppUserStatus,
    salt_hex: String,
    hash_hex: String,
    iterations: u32,
    updated_at: i64,
}

fn auth_error(code: &str, message: impl Into<String>) -> HostError {
    HostError::new(code, message, false)
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn role_to_str(role: AppUserRole) -> &'static str {
    match role {
        AppUserRole::Admin => "admin",
        AppUserRole::Member => "member",
    }
}

fn role_from_str(value: &str) -> AppUserRole {
    match value {
        "admin" => AppUserRole::Admin,
        _ => AppUserRole::Member,
    }
}

fn status_to_str(status: AppUserStatus) -> &'static str {
    match status {
        AppUserStatus::Active => "active",
        AppUserStatus::Disabled => "disabled",
    }
}

fn status_from_str(value: &str) -> AppUserStatus {
    match value {
        "disabled" => AppUserStatus::Disabled,
        _ => AppUserStatus::Active,
    }
}

pub(crate) fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut key_block = [0u8; 64];
    if key.len() > 64 {
        let digest = Sha256::digest(key);
        key_block[..32].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0u8; 64];
    let mut opad = [0u8; 64];
    for index in 0..64 {
        ipad[index] = key_block[index] ^ 0x36;
        opad[index] = key_block[index] ^ 0x5c;
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    for part in parts {
        inner.update(part);
    }
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    let mut out = [0u8; 32];
    out.copy_from_slice(&outer.finalize());
    out
}

pub(crate) fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    // Derived key length equals one SHA-256 block, so only F(P, S, c, 1) is needed.
    let iterations = iterations.max(1);
    let mut block = hmac_sha256(password, &[salt, &1u32.to_be_bytes()]);
    let mut result = block;
    for _ in 1..iterations {
        block = hmac_sha256(password, &[&block]);
        for (accumulator, next) in result.iter_mut().zip(block.iter()) {
            *accumulator ^= next;
        }
    }
    result
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

fn random_salt_hex() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn derive_hash_hex(password: &str, salt_hex: &str, iterations: u32) -> Result<String, HostError> {
    let salt = hex_decode(salt_hex)
        .ok_or_else(|| HostError::internal("stored password salt is not valid hex"))?;
    Ok(hex_encode(&pbkdf2_sha256(
        password.as_bytes(),
        &salt,
        iterations,
    )))
}

fn validate_password(password: &str) -> Result<(), HostError> {
    let chars = password.chars().count();
    if chars < MIN_PASSWORD_CHARS {
        return Err(auth_error(
            "AUTH_WEAK_PASSWORD",
            format!("password must be at least {MIN_PASSWORD_CHARS} characters"),
        ));
    }
    if chars > MAX_PASSWORD_CHARS {
        return Err(auth_error(
            "AUTH_WEAK_PASSWORD",
            format!("password must be at most {MAX_PASSWORD_CHARS} characters"),
        ));
    }
    Ok(())
}

fn normalize_username(username: &str) -> Result<String, HostError> {
    let trimmed = username.trim();
    let chars = trimmed.chars().count();
    if !(2..=MAX_USERNAME_CHARS).contains(&chars) {
        return Err(auth_error(
            "AUTH_INVALID_USERNAME",
            format!("username must be 2..{MAX_USERNAME_CHARS} characters"),
        ));
    }
    if trimmed
        .chars()
        .any(|character| character.is_control() || "/\\\"'`<>".contains(character))
    {
        return Err(auth_error(
            "AUTH_INVALID_USERNAME",
            "username contains unsupported characters",
        ));
    }
    Ok(trimmed.to_string())
}

impl AuthService {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Result<Self, HostError> {
        {
            let guard = connection
                .lock()
                .map_err(|_| HostError::internal("auth SQLite lock is poisoned"))?;
            guard
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS app_users (
                        username TEXT PRIMARY KEY,
                        role TEXT NOT NULL,
                        status TEXT NOT NULL,
                        salt_hex TEXT NOT NULL,
                        hash_hex TEXT NOT NULL,
                        iterations INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL
                    );
                    CREATE TABLE IF NOT EXISTS app_auth_meta (
                        key TEXT PRIMARY KEY,
                        value TEXT NOT NULL
                    );",
                )
                .map_err(|error| {
                    HostError::internal(format!("initialize auth schema failed: {error}"))
                })?;
        }
        let (registry, feedback) = match RegistryStore::from_env() {
            RegistryStoreLoad::Configured(store) => (
                Some(store),
                RegistryFeedback {
                    sync: AuthRegistrySync::Degraded,
                    message: Some("尚未与云端同步，登录或刷新时会自动同步".to_string()),
                },
            ),
            RegistryStoreLoad::Unconfigured => (
                None,
                RegistryFeedback {
                    sync: AuthRegistrySync::LocalOnly,
                    message: None,
                },
            ),
            RegistryStoreLoad::Invalid(reason) => (
                None,
                RegistryFeedback {
                    sync: AuthRegistrySync::Degraded,
                    message: Some(format!("云备份配置有误：{reason}")),
                },
            ),
        };
        Ok(Self {
            connection,
            registry,
            registry_state: Mutex::new(feedback),
            session: Mutex::new(None),
        })
    }

    // ---- meta helpers -----------------------------------------------------

    fn meta_get(connection: &Connection, key: &str) -> Result<Option<String>, HostError> {
        connection
            .query_row(
                "SELECT value FROM app_auth_meta WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| HostError::internal(format!("read auth meta failed: {error}")))
    }

    fn meta_set(connection: &Connection, key: &str, value: &str) -> Result<(), HostError> {
        connection
            .execute(
                "INSERT INTO app_auth_meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|error| HostError::internal(format!("write auth meta failed: {error}")))
    }

    fn registry_revision(connection: &Connection) -> Result<i64, HostError> {
        Ok(Self::meta_get(connection, META_REGISTRY_REVISION)?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0))
    }

    fn registry_dirty(connection: &Connection) -> Result<bool, HostError> {
        Ok(Self::meta_get(connection, META_REGISTRY_DIRTY)?.as_deref() == Some("1"))
    }

    fn load_users(connection: &Connection) -> Result<Vec<StoredUser>, HostError> {
        let mut statement = connection
            .prepare(
                "SELECT username, role, status, salt_hex, hash_hex, iterations, updated_at
                 FROM app_users ORDER BY username",
            )
            .map_err(|error| HostError::internal(format!("prepare user query failed: {error}")))?;
        let rows = statement
            .query_map([], |row| {
                Ok(StoredUser {
                    username: row.get(0)?,
                    role: role_from_str(&row.get::<_, String>(1)?),
                    status: status_from_str(&row.get::<_, String>(2)?),
                    salt_hex: row.get(3)?,
                    hash_hex: row.get(4)?,
                    iterations: row.get::<_, i64>(5)? as u32,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|error| HostError::internal(format!("query users failed: {error}")))?;
        let mut users = Vec::new();
        for row in rows {
            users.push(
                row.map_err(|error| HostError::internal(format!("read user row failed: {error}")))?,
            );
        }
        Ok(users)
    }

    fn load_user(connection: &Connection, username: &str) -> Result<Option<StoredUser>, HostError> {
        connection
            .query_row(
                "SELECT username, role, status, salt_hex, hash_hex, iterations, updated_at
                 FROM app_users WHERE username = ?1",
                params![username],
                |row| {
                    Ok(StoredUser {
                        username: row.get(0)?,
                        role: role_from_str(&row.get::<_, String>(1)?),
                        status: status_from_str(&row.get::<_, String>(2)?),
                        salt_hex: row.get(3)?,
                        hash_hex: row.get(4)?,
                        iterations: row.get::<_, i64>(5)? as u32,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|error| HostError::internal(format!("read user failed: {error}")))
    }

    fn user_count(connection: &Connection) -> Result<u32, HostError> {
        connection
            .query_row("SELECT COUNT(*) FROM app_users", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|count| count as u32)
            .map_err(|error| HostError::internal(format!("count users failed: {error}")))
    }

    fn set_feedback(&self, sync: AuthRegistrySync, message: Option<String>) {
        if let Ok(mut state) = self.registry_state.lock() {
            *state = RegistryFeedback { sync, message };
        }
    }

    fn feedback(&self) -> RegistryFeedback {
        self.registry_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(RegistryFeedback {
                sync: AuthRegistrySync::LocalOnly,
                message: None,
            })
    }

    // ---- registry sync ----------------------------------------------------

    /// Pulls the shared registry (best effort) and reconciles by revision.
    /// Local mutations always survive: a lower remote revision triggers a
    /// push instead of a rollback.
    fn sync_registry(&self) {
        let Some(registry) = self.registry.as_ref() else {
            return;
        };
        let remote = match registry.get_object(REGISTRY_OBJECT) {
            Ok(remote) => remote,
            Err(error) => {
                self.set_feedback(
                    AuthRegistrySync::Degraded,
                    Some(format!("云端账号表读取失败：{error}")),
                );
                return;
            }
        };
        let parsed = remote.and_then(|bytes| {
            serde_json::from_slice::<RegistryDocument>(&bytes)
                .map_err(|error| {
                    self.set_feedback(
                        AuthRegistrySync::Degraded,
                        Some(format!("云端账号表格式有误：{error}")),
                    );
                })
                .ok()
        });

        let (local_revision, dirty, local_count) = {
            let Ok(connection) = self.connection.lock() else {
                return;
            };
            let revision = Self::registry_revision(&connection).unwrap_or(0);
            let dirty = Self::registry_dirty(&connection).unwrap_or(false);
            let count = Self::user_count(&connection).unwrap_or(0);
            (revision, dirty, count)
        };

        match parsed {
            Some(document) if document.revision > local_revision => {
                // Remote is ahead: adopt it wholesale.
                let Ok(connection) = self.connection.lock() else {
                    return;
                };
                let adopt = (|| -> Result<(), HostError> {
                    connection
                        .execute("DELETE FROM app_users", [])
                        .map_err(|error| {
                            HostError::internal(format!("clear users failed: {error}"))
                        })?;
                    for user in &document.users {
                        connection
                            .execute(
                                "INSERT INTO app_users
                                 (username, role, status, salt_hex, hash_hex, iterations, updated_at)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                                params![
                                    user.username,
                                    user.role,
                                    user.status,
                                    user.salt_hex,
                                    user.hash_hex,
                                    user.iterations as i64,
                                    user.updated_at
                                ],
                            )
                            .map_err(|error| {
                                HostError::internal(format!("adopt remote user failed: {error}"))
                            })?;
                    }
                    Self::meta_set(
                        &connection,
                        META_REGISTRY_REVISION,
                        &document.revision.to_string(),
                    )?;
                    Self::meta_set(&connection, META_REGISTRY_DIRTY, "0")?;
                    Ok(())
                })();
                match adopt {
                    Ok(()) => self.set_feedback(AuthRegistrySync::Synced, None),
                    Err(error) => self.set_feedback(
                        AuthRegistrySync::Degraded,
                        Some(format!("云端账号表落地失败:{}", error.message)),
                    ),
                }
            }
            Some(document) if document.revision < local_revision || dirty => {
                self.push_registry(local_revision.max(document.revision));
            }
            Some(_) => {
                self.set_feedback(AuthRegistrySync::Synced, None);
            }
            None => {
                // Nothing (valid) in the cloud yet. Publish local users if any.
                if local_count > 0 {
                    self.push_registry(local_revision.max(1));
                } else {
                    self.set_feedback(AuthRegistrySync::Synced, None);
                }
            }
        }
    }

    /// Serializes the local table and writes it to the shared registry with
    /// `revision`. Local data is already committed when this runs; failures
    /// only mark the registry dirty for a later retry.
    fn push_registry(&self, revision: i64) {
        let Some(registry) = self.registry.as_ref() else {
            return;
        };
        let document = {
            let Ok(connection) = self.connection.lock() else {
                return;
            };
            let users = match Self::load_users(&connection) {
                Ok(users) => users,
                Err(error) => {
                    self.set_feedback(
                        AuthRegistrySync::Degraded,
                        Some(format!("读取本地账号失败:{}", error.message)),
                    );
                    return;
                }
            };
            RegistryDocument {
                version: REGISTRY_VERSION,
                revision,
                updated_at: now_millis(),
                users: users
                    .iter()
                    .map(|user| RegistryUser {
                        username: user.username.clone(),
                        role: role_to_str(user.role).to_string(),
                        status: status_to_str(user.status).to_string(),
                        salt_hex: user.salt_hex.clone(),
                        hash_hex: user.hash_hex.clone(),
                        iterations: user.iterations,
                        updated_at: user.updated_at,
                    })
                    .collect(),
            }
        };
        let bytes = match serde_json::to_vec(&document) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.set_feedback(
                    AuthRegistrySync::Degraded,
                    Some(format!("账号表序列化失败：{error}")),
                );
                return;
            }
        };
        match registry.put_object(REGISTRY_OBJECT, bytes) {
            Ok(()) => {
                if let Ok(connection) = self.connection.lock() {
                    let _ =
                        Self::meta_set(&connection, META_REGISTRY_REVISION, &revision.to_string());
                    let _ = Self::meta_set(&connection, META_REGISTRY_DIRTY, "0");
                }
                self.set_feedback(AuthRegistrySync::Synced, None);
            }
            Err(error) => {
                if let Ok(connection) = self.connection.lock() {
                    let _ = Self::meta_set(&connection, META_REGISTRY_DIRTY, "1");
                }
                self.set_feedback(
                    AuthRegistrySync::Degraded,
                    Some(format!(
                        "本机已生效，云端同步失败（联网后可在设置刷新）：{error}"
                    )),
                );
            }
        }
    }

    /// Bumps the local revision after a mutation and pushes best-effort.
    fn after_mutation(&self) {
        let next_revision = {
            let Ok(connection) = self.connection.lock() else {
                return;
            };
            let revision = Self::registry_revision(&connection).unwrap_or(0) + 1;
            let _ = Self::meta_set(&connection, META_REGISTRY_REVISION, &revision.to_string());
            let _ = Self::meta_set(&connection, META_REGISTRY_DIRTY, "1");
            revision
        };
        self.push_registry(next_revision);
    }

    // ---- session helpers --------------------------------------------------

    fn session_username(&self) -> Option<String> {
        self.session.lock().ok().and_then(|session| session.clone())
    }

    fn require_login(&self) -> Result<String, HostError> {
        self.session_username()
            .ok_or_else(|| auth_error("AUTH_NOT_LOGGED_IN", "please log in first"))
    }

    fn clear_session_for(&self, username: &str) {
        if let Ok(mut session) = self.session.lock() {
            if session.as_deref() == Some(username) {
                *session = None;
            }
        }
    }

    pub(crate) fn require_active_principal(&self) -> Result<AuthPrincipal, HostError> {
        let username = self.require_login()?;
        let user = {
            let connection = self
                .connection
                .lock()
                .map_err(|_| HostError::internal("auth SQLite lock is poisoned"))?;
            Self::load_user(&connection, &username)?
        };
        let Some(user) = user else {
            self.clear_session_for(&username);
            return Err(auth_error(
                "AUTH_NOT_LOGGED_IN",
                "session user no longer exists",
            ));
        };
        if user.status != AppUserStatus::Active {
            self.clear_session_for(&username);
            return Err(auth_error("AUTH_USER_DISABLED", "this account is disabled"));
        }
        Ok(AuthPrincipal {
            username: user.username,
            role: user.role,
        })
    }

    pub(crate) fn require_active_admin(&self) -> Result<AuthPrincipal, HostError> {
        let principal = self.require_active_principal()?;
        if principal.role != AppUserRole::Admin {
            return Err(auth_error(
                "AUTH_FORBIDDEN",
                "only an administrator can manage users",
            ));
        }
        Ok(principal)
    }

    fn require_admin(&self) -> Result<String, HostError> {
        self.require_active_admin()
            .map(|principal| principal.username)
    }

    fn public_record(user: &StoredUser) -> AppUserRecord {
        AppUserRecord {
            username: user.username.clone(),
            role: user.role,
            status: user.status,
            updated_at: user.updated_at,
        }
    }

    // ---- public surface ---------------------------------------------------

    pub fn status(&self) -> AuthStatus {
        let (initialized, user_count, revision, current_user) = {
            match self.connection.lock() {
                Ok(connection) => {
                    let count = Self::user_count(&connection).unwrap_or(0);
                    let revision = Self::registry_revision(&connection).unwrap_or(0);
                    let current = self.session_username().and_then(|username| {
                        Self::load_user(&connection, &username)
                            .ok()
                            .flatten()
                            .map(|user| Self::public_record(&user))
                    });
                    (count > 0, count, revision, current)
                }
                Err(_) => (false, 0, 0, None),
            }
        };
        let feedback = self.feedback();
        AuthStatus {
            initialized,
            current_user,
            registry_sync: feedback.sync,
            registry_message: feedback.message,
            registry_revision: revision,
            user_count,
        }
    }

    pub fn initialize_admin(&self, credentials: AuthCredentials) -> Result<AuthStatus, HostError> {
        let username = normalize_username(&credentials.username)?;
        validate_password(&credentials.password)?;
        // Another machine may have initialized the company already.
        self.sync_registry();
        {
            let connection = self
                .connection
                .lock()
                .map_err(|_| HostError::internal("auth SQLite lock is poisoned"))?;
            if Self::user_count(&connection)? > 0 {
                return Err(auth_error(
                    "AUTH_ALREADY_INITIALIZED",
                    "accounts already exist; please log in instead",
                ));
            }
            let salt_hex = random_salt_hex();
            let hash_hex = derive_hash_hex(&credentials.password, &salt_hex, PBKDF2_ITERATIONS)?;
            connection
                .execute(
                    "INSERT INTO app_users
                     (username, role, status, salt_hex, hash_hex, iterations, updated_at)
                     VALUES (?1, 'admin', 'active', ?2, ?3, ?4, ?5)",
                    params![
                        username,
                        salt_hex,
                        hash_hex,
                        PBKDF2_ITERATIONS as i64,
                        now_millis()
                    ],
                )
                .map_err(|error| {
                    HostError::internal(format!("create administrator failed: {error}"))
                })?;
        }
        if let Ok(mut session) = self.session.lock() {
            *session = Some(username);
        }
        self.after_mutation();
        Ok(self.status())
    }

    pub fn login(&self, credentials: AuthCredentials) -> Result<AuthStatus, HostError> {
        let username = credentials.username.trim().to_string();
        // Refresh the shared registry first so new/removed accounts apply.
        self.sync_registry();
        let user = {
            let connection = self
                .connection
                .lock()
                .map_err(|_| HostError::internal("auth SQLite lock is poisoned"))?;
            if Self::user_count(&connection)? == 0 {
                return Err(auth_error(
                    "AUTH_NOT_INITIALIZED",
                    "no accounts exist yet; create the administrator first",
                ));
            }
            Self::load_user(&connection, &username)?
        };
        let Some(user) = user else {
            return Err(auth_error(
                "AUTH_INVALID_CREDENTIALS",
                "username or password is incorrect",
            ));
        };
        if user.status == AppUserStatus::Disabled {
            return Err(auth_error("AUTH_USER_DISABLED", "this account is disabled"));
        }
        let expected = hex_decode(&user.hash_hex)
            .ok_or_else(|| HostError::internal("stored password hash is not valid hex"))?;
        let salt = hex_decode(&user.salt_hex)
            .ok_or_else(|| HostError::internal("stored password salt is not valid hex"))?;
        let candidate = pbkdf2_sha256(credentials.password.as_bytes(), &salt, user.iterations);
        if !constant_time_eq(&candidate, &expected) {
            return Err(auth_error(
                "AUTH_INVALID_CREDENTIALS",
                "username or password is incorrect",
            ));
        }
        if let Ok(mut session) = self.session.lock() {
            *session = Some(user.username.clone());
        }
        Ok(self.status())
    }

    pub fn logout(&self) -> AuthStatus {
        if let Ok(mut session) = self.session.lock() {
            *session = None;
        }
        self.status()
    }

    pub fn change_password(
        &self,
        payload: AuthChangePasswordPayload,
    ) -> Result<AuthStatus, HostError> {
        let username = self.require_active_principal()?.username;
        validate_password(&payload.new_password)?;
        {
            let connection = self
                .connection
                .lock()
                .map_err(|_| HostError::internal("auth SQLite lock is poisoned"))?;
            let user = Self::load_user(&connection, &username)?
                .ok_or_else(|| auth_error("AUTH_NOT_LOGGED_IN", "session user no longer exists"))?;
            let expected = hex_decode(&user.hash_hex)
                .ok_or_else(|| HostError::internal("stored password hash is not valid hex"))?;
            let salt = hex_decode(&user.salt_hex)
                .ok_or_else(|| HostError::internal("stored password salt is not valid hex"))?;
            let candidate = pbkdf2_sha256(payload.old_password.as_bytes(), &salt, user.iterations);
            if !constant_time_eq(&candidate, &expected) {
                return Err(auth_error(
                    "AUTH_INVALID_CREDENTIALS",
                    "the current password is incorrect",
                ));
            }
            let salt_hex = random_salt_hex();
            let hash_hex = derive_hash_hex(&payload.new_password, &salt_hex, PBKDF2_ITERATIONS)?;
            connection
                .execute(
                    "UPDATE app_users
                     SET salt_hex = ?2, hash_hex = ?3, iterations = ?4, updated_at = ?5
                     WHERE username = ?1",
                    params![
                        username,
                        salt_hex,
                        hash_hex,
                        PBKDF2_ITERATIONS as i64,
                        now_millis()
                    ],
                )
                .map_err(|error| HostError::internal(format!("update password failed: {error}")))?;
        }
        self.after_mutation();
        Ok(self.status())
    }

    fn users_snapshot(&self) -> Result<AuthUsersSnapshot, HostError> {
        let (users, revision) = {
            let connection = self
                .connection
                .lock()
                .map_err(|_| HostError::internal("auth SQLite lock is poisoned"))?;
            let users = Self::load_users(&connection)?
                .iter()
                .map(Self::public_record)
                .collect();
            (users, Self::registry_revision(&connection)?)
        };
        let feedback = self.feedback();
        Ok(AuthUsersSnapshot {
            users,
            registry_sync: feedback.sync,
            registry_message: feedback.message,
            registry_revision: revision,
        })
    }

    pub fn list_users(&self) -> Result<AuthUsersSnapshot, HostError> {
        self.require_admin()?;
        self.sync_registry();
        self.users_snapshot()
    }

    pub fn create_user(
        &self,
        payload: AuthCreateUserPayload,
    ) -> Result<AuthUsersSnapshot, HostError> {
        self.require_admin()?;
        let username = normalize_username(&payload.username)?;
        validate_password(&payload.password)?;
        self.sync_registry();
        {
            let connection = self
                .connection
                .lock()
                .map_err(|_| HostError::internal("auth SQLite lock is poisoned"))?;
            if Self::user_count(&connection)? as usize >= MAX_USERS {
                return Err(auth_error("AUTH_TOO_MANY_USERS", "too many accounts"));
            }
            if Self::load_user(&connection, &username)?.is_some() {
                return Err(auth_error(
                    "AUTH_USER_EXISTS",
                    "this username already exists",
                ));
            }
            let salt_hex = random_salt_hex();
            let hash_hex = derive_hash_hex(&payload.password, &salt_hex, PBKDF2_ITERATIONS)?;
            connection
                .execute(
                    "INSERT INTO app_users
                     (username, role, status, salt_hex, hash_hex, iterations, updated_at)
                     VALUES (?1, ?2, 'active', ?3, ?4, ?5, ?6)",
                    params![
                        username,
                        role_to_str(payload.role),
                        salt_hex,
                        hash_hex,
                        PBKDF2_ITERATIONS as i64,
                        now_millis()
                    ],
                )
                .map_err(|error| HostError::internal(format!("create user failed: {error}")))?;
        }
        self.after_mutation();
        self.users_snapshot()
    }

    pub fn reset_password(
        &self,
        payload: AuthResetPasswordPayload,
    ) -> Result<AuthUsersSnapshot, HostError> {
        self.require_admin()?;
        validate_password(&payload.new_password)?;
        {
            let connection = self
                .connection
                .lock()
                .map_err(|_| HostError::internal("auth SQLite lock is poisoned"))?;
            if Self::load_user(&connection, &payload.username)?.is_none() {
                return Err(auth_error(
                    "AUTH_USER_NOT_FOUND",
                    "this account does not exist",
                ));
            }
            let salt_hex = random_salt_hex();
            let hash_hex = derive_hash_hex(&payload.new_password, &salt_hex, PBKDF2_ITERATIONS)?;
            connection
                .execute(
                    "UPDATE app_users
                     SET salt_hex = ?2, hash_hex = ?3, iterations = ?4, updated_at = ?5
                     WHERE username = ?1",
                    params![
                        payload.username,
                        salt_hex,
                        hash_hex,
                        PBKDF2_ITERATIONS as i64,
                        now_millis()
                    ],
                )
                .map_err(|error| HostError::internal(format!("reset password failed: {error}")))?;
        }
        self.after_mutation();
        self.users_snapshot()
    }

    pub fn delete_user(
        &self,
        payload: AuthDeleteUserPayload,
    ) -> Result<AuthUsersSnapshot, HostError> {
        let operator = self.require_admin()?;
        if operator == payload.username {
            return Err(auth_error(
                "AUTH_SELF_DELETE",
                "you cannot delete the account you are logged in with",
            ));
        }
        {
            let connection = self
                .connection
                .lock()
                .map_err(|_| HostError::internal("auth SQLite lock is poisoned"))?;
            let Some(target) = Self::load_user(&connection, &payload.username)? else {
                return Err(auth_error(
                    "AUTH_USER_NOT_FOUND",
                    "this account does not exist",
                ));
            };
            if target.role == AppUserRole::Admin {
                let admin_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM app_users WHERE role = 'admin'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|error| {
                        HostError::internal(format!("count administrators failed: {error}"))
                    })?;
                if admin_count <= 1 {
                    return Err(auth_error(
                        "AUTH_LAST_ADMIN",
                        "the last administrator cannot be deleted",
                    ));
                }
            }
            connection
                .execute(
                    "DELETE FROM app_users WHERE username = ?1",
                    params![payload.username],
                )
                .map_err(|error| HostError::internal(format!("delete user failed: {error}")))?;
        }
        self.after_mutation();
        self.users_snapshot()
    }

    pub fn refresh_registry(&self) -> AuthStatus {
        self.sync_registry();
        self.status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> AuthService {
        let connection = Connection::open_in_memory().expect("open in-memory SQLite");
        let mut service =
            AuthService::new(Arc::new(Mutex::new(connection))).expect("create auth service");
        service.registry = None;
        service.set_feedback(AuthRegistrySync::LocalOnly, None);
        service
    }

    #[test]
    fn pbkdf2_sha256_matches_known_vectors() {
        // Standard PBKDF2-HMAC-SHA256 vectors (password/salt, dkLen = 32).
        let one = pbkdf2_sha256(b"password", b"salt", 1);
        assert_eq!(
            hex_encode(&one),
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );
        let two = pbkdf2_sha256(b"password", b"salt", 2);
        assert_eq!(
            hex_encode(&two),
            "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"
        );
        let many = pbkdf2_sha256(b"password", b"salt", 4096);
        assert_eq!(
            hex_encode(&many),
            "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a"
        );
    }

    #[test]
    fn initialize_login_and_password_lifecycle() {
        let auth = service();
        let status = auth.status();
        assert!(!status.initialized);

        let status = auth
            .initialize_admin(AuthCredentials {
                username: "老板".to_string(),
                password: "123456".to_string(),
            })
            .expect("initialize admin");
        assert!(status.initialized);
        assert_eq!(
            status
                .current_user
                .as_ref()
                .map(|user| user.username.as_str()),
            Some("老板")
        );

        // Second initialization is rejected.
        let error = auth
            .initialize_admin(AuthCredentials {
                username: "again".to_string(),
                password: "123456".to_string(),
            })
            .unwrap_err();
        assert_eq!(error.code, "AUTH_ALREADY_INITIALIZED");

        auth.logout();
        let error = auth
            .login(AuthCredentials {
                username: "老板".to_string(),
                password: "wrong-1".to_string(),
            })
            .unwrap_err();
        assert_eq!(error.code, "AUTH_INVALID_CREDENTIALS");

        let status = auth
            .login(AuthCredentials {
                username: "老板".to_string(),
                password: "123456".to_string(),
            })
            .expect("login");
        assert!(status.current_user.is_some());

        auth.change_password(AuthChangePasswordPayload {
            old_password: "123456".to_string(),
            new_password: "abcdef".to_string(),
        })
        .expect("change password");
        auth.logout();
        assert!(auth
            .login(AuthCredentials {
                username: "老板".to_string(),
                password: "abcdef".to_string(),
            })
            .is_ok());
    }

    #[test]
    fn admin_manages_users_with_guardrails() {
        let auth = service();
        auth.initialize_admin(AuthCredentials {
            username: "admin".to_string(),
            password: "123456".to_string(),
        })
        .expect("initialize admin");

        let snapshot = auth
            .create_user(AuthCreateUserPayload {
                username: "市场部小李".to_string(),
                password: "654321".to_string(),
                role: AppUserRole::Member,
            })
            .expect("create user");
        assert_eq!(snapshot.users.len(), 2);

        // Duplicate username is rejected.
        let error = auth
            .create_user(AuthCreateUserPayload {
                username: "市场部小李".to_string(),
                password: "654321".to_string(),
                role: AppUserRole::Member,
            })
            .unwrap_err();
        assert_eq!(error.code, "AUTH_USER_EXISTS");

        // Weak password is rejected.
        let error = auth
            .create_user(AuthCreateUserPayload {
                username: "short".to_string(),
                password: "123".to_string(),
                role: AppUserRole::Member,
            })
            .unwrap_err();
        assert_eq!(error.code, "AUTH_WEAK_PASSWORD");

        // Reset password then log in as the member.
        auth.reset_password(AuthResetPasswordPayload {
            username: "市场部小李".to_string(),
            new_password: "777777".to_string(),
        })
        .expect("reset password");

        // Admin cannot delete self; last admin is protected.
        let error = auth
            .delete_user(AuthDeleteUserPayload {
                username: "admin".to_string(),
            })
            .unwrap_err();
        assert_eq!(error.code, "AUTH_SELF_DELETE");

        auth.logout();
        auth.login(AuthCredentials {
            username: "市场部小李".to_string(),
            password: "777777".to_string(),
        })
        .expect("member login");

        // Member cannot manage users.
        let error = auth.list_users().unwrap_err();
        assert_eq!(error.code, "AUTH_FORBIDDEN");

        // Back to admin: delete the member; deleted member can no longer log in.
        auth.logout();
        auth.login(AuthCredentials {
            username: "admin".to_string(),
            password: "123456".to_string(),
        })
        .expect("admin login");
        let snapshot = auth
            .delete_user(AuthDeleteUserPayload {
                username: "市场部小李".to_string(),
            })
            .expect("delete user");
        assert_eq!(snapshot.users.len(), 1);
        auth.logout();
        let error = auth
            .login(AuthCredentials {
                username: "市场部小李".to_string(),
                password: "777777".to_string(),
            })
            .unwrap_err();
        assert_eq!(error.code, "AUTH_INVALID_CREDENTIALS");
    }

    #[test]
    fn active_principal_requires_a_logged_in_active_user() {
        let auth = service();
        let error = auth.require_active_principal().unwrap_err();
        assert_eq!(error.code, "AUTH_NOT_LOGGED_IN");

        auth.initialize_admin(AuthCredentials {
            username: "admin".to_string(),
            password: "123456".to_string(),
        })
        .expect("initialize admin");
        let admin = auth
            .require_active_principal()
            .expect("active admin principal");
        assert_eq!(admin.username, "admin");
        assert_eq!(admin.role, AppUserRole::Admin);
        assert_eq!(
            auth.require_active_admin().expect("active administrator"),
            admin
        );

        auth.create_user(AuthCreateUserPayload {
            username: "member".to_string(),
            password: "654321".to_string(),
            role: AppUserRole::Member,
        })
        .expect("create member");
        auth.logout();
        auth.login(AuthCredentials {
            username: "member".to_string(),
            password: "654321".to_string(),
        })
        .expect("member login");
        let member = auth
            .require_active_principal()
            .expect("active member principal");
        assert_eq!(member.username, "member");
        assert_eq!(member.role, AppUserRole::Member);
        let error = auth.require_active_admin().unwrap_err();
        assert_eq!(error.code, "AUTH_FORBIDDEN");
        assert_eq!(auth.session_username().as_deref(), Some("member"));
    }

    #[test]
    fn disabled_or_deleted_session_user_is_invalidated_immediately() {
        let auth = service();
        auth.initialize_admin(AuthCredentials {
            username: "admin".to_string(),
            password: "123456".to_string(),
        })
        .expect("initialize admin");

        {
            let connection = auth.connection.lock().expect("lock auth SQLite");
            connection
                .execute(
                    "UPDATE app_users SET status = 'disabled' WHERE username = 'admin'",
                    [],
                )
                .expect("disable current user");
        }
        let error = auth.require_active_principal().unwrap_err();
        assert_eq!(error.code, "AUTH_USER_DISABLED");
        assert_eq!(auth.session_username(), None);

        {
            let connection = auth.connection.lock().expect("lock auth SQLite");
            connection
                .execute(
                    "UPDATE app_users SET status = 'active' WHERE username = 'admin'",
                    [],
                )
                .expect("reactivate current user");
        }
        auth.login(AuthCredentials {
            username: "admin".to_string(),
            password: "123456".to_string(),
        })
        .expect("admin login");
        {
            let connection = auth.connection.lock().expect("lock auth SQLite");
            connection
                .execute("DELETE FROM app_users WHERE username = 'admin'", [])
                .expect("delete current user");
        }
        let error = auth.require_active_principal().unwrap_err();
        assert_eq!(error.code, "AUTH_NOT_LOGGED_IN");
        assert_eq!(auth.session_username(), None);
    }

    #[test]
    fn role_downgrade_revokes_admin_access_immediately() {
        let auth = service();
        auth.initialize_admin(AuthCredentials {
            username: "admin".to_string(),
            password: "123456".to_string(),
        })
        .expect("initialize admin");
        auth.require_active_admin().expect("active administrator");

        {
            let connection = auth.connection.lock().expect("lock auth SQLite");
            connection
                .execute(
                    "UPDATE app_users SET role = 'member' WHERE username = 'admin'",
                    [],
                )
                .expect("downgrade current user");
        }
        let error = auth.require_active_admin().unwrap_err();
        assert_eq!(error.code, "AUTH_FORBIDDEN");
        let principal = auth
            .require_active_principal()
            .expect("downgraded active principal");
        assert_eq!(principal.role, AppUserRole::Member);
    }

    #[test]
    fn disabled_session_cannot_change_password() {
        let auth = service();
        auth.initialize_admin(AuthCredentials {
            username: "admin".to_string(),
            password: "123456".to_string(),
        })
        .expect("initialize admin");
        {
            let connection = auth.connection.lock().expect("lock auth SQLite");
            connection
                .execute(
                    "UPDATE app_users SET status = 'disabled' WHERE username = 'admin'",
                    [],
                )
                .expect("disable current user");
        }

        let error = auth
            .change_password(AuthChangePasswordPayload {
                old_password: "123456".to_string(),
                new_password: "654321".to_string(),
            })
            .unwrap_err();

        assert_eq!(error.code, "AUTH_USER_DISABLED");
        assert_eq!(auth.session_username(), None);
    }
}
