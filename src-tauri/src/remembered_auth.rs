use crate::protocol::{AuthCredentials, HostError};
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "com.banshan.aigc.desktop";
const KEYRING_ACCOUNT: &str = "remembered-login";
const MAX_USERNAME_BYTES: usize = 256;
const MAX_PASSWORD_BYTES: usize = 2048;

#[cfg(any(windows, target_os = "macos"))]
fn entry() -> Result<keyring::Entry, HostError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(|_| storage_error())
}

#[cfg(any(windows, target_os = "macos"))]
pub(crate) fn load() -> Result<Option<AuthCredentials>, HostError> {
    let secret = match entry()?.get_password() {
        Ok(secret) => Zeroizing::new(secret),
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(_) => return Err(storage_error()),
    };
    let credentials: AuthCredentials =
        serde_json::from_str(secret.as_str()).map_err(|_| corrupt_error())?;
    validate(&credentials)?;
    Ok(Some(credentials))
}

#[cfg(any(windows, target_os = "macos"))]
pub(crate) fn save(mut credentials: AuthCredentials) -> Result<(), HostError> {
    credentials.username = credentials.username.trim().to_string();
    validate(&credentials)?;
    let secret = Zeroizing::new(serde_json::to_string(&credentials).map_err(|_| storage_error())?);
    entry()?
        .set_password(secret.as_str())
        .map_err(|_| storage_error())
}

#[cfg(any(windows, target_os = "macos"))]
pub(crate) fn clear() -> Result<(), HostError> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(storage_error()),
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn load() -> Result<Option<AuthCredentials>, HostError> {
    Err(unsupported_error())
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn save(_credentials: AuthCredentials) -> Result<(), HostError> {
    Err(unsupported_error())
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn clear() -> Result<(), HostError> {
    Err(unsupported_error())
}

fn validate(credentials: &AuthCredentials) -> Result<(), HostError> {
    let username = credentials.username.trim();
    if username.is_empty() || credentials.password.is_empty() {
        return Err(HostError::validation("账号和密码不能为空"));
    }
    if username.len() > MAX_USERNAME_BYTES || credentials.password.len() > MAX_PASSWORD_BYTES {
        return Err(HostError::validation("账号或密码过长，无法安全保存"));
    }
    Ok(())
}

fn storage_error() -> HostError {
    HostError::new(
        "AUTH_REMEMBER_STORAGE_FAILED",
        "系统安全存储不可用，无法记住账号密码。取消勾选后仍可正常登录。",
        true,
    )
}

fn corrupt_error() -> HostError {
    HostError::new(
        "AUTH_REMEMBERED_CREDENTIALS_CORRUPT",
        "已保存的登录信息无法读取，请取消勾选后重新登录。",
        false,
    )
}

#[cfg(not(any(windows, target_os = "macos")))]
fn unsupported_error() -> HostError {
    HostError::new(
        "AUTH_REMEMBER_UNSUPPORTED",
        "当前系统暂不支持安全保存登录信息。",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembered_credentials_require_both_fields() {
        assert!(validate(&AuthCredentials {
            username: "member".to_string(),
            password: "123456".to_string(),
        })
        .is_ok());
        assert!(validate(&AuthCredentials {
            username: " ".to_string(),
            password: "123456".to_string(),
        })
        .is_err());
        assert!(validate(&AuthCredentials {
            username: "member".to_string(),
            password: String::new(),
        })
        .is_err());
    }
}
