use crate::protocol::CodexProbeStatus;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);
pub(crate) const REQUIRED_CODEX_VERSION: &str = "0.144.5";
const PRODUCT_PROVIDER_ID: &str = "bsaigc";
const DEFAULT_PROVIDER_BASE_URL: &str = "https://api.openai.com/v1";
const MANAGED_HOME_DIR: &str = "codex-home";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) struct Candidate {
    pub(crate) path: PathBuf,
    pub(crate) source: String,
}

#[derive(Clone)]
pub(crate) struct CodexLaunchConfig {
    pub(crate) codex_home: PathBuf,
    pub(crate) working_directory: PathBuf,
    environment: Vec<(OsString, OsString)>,
    api_key: Option<Zeroizing<String>>,
}

impl std::fmt::Debug for CodexLaunchConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexLaunchConfig")
            .field("codex_home", &self.codex_home)
            .field("working_directory", &self.working_directory)
            .field(
                "environment_names",
                &self
                    .environment
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>(),
            )
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

pub fn probe_codex() -> CodexProbeStatus {
    let workspace = env::temp_dir()
        .join("bsaigc-desktop")
        .join("codex-probe")
        .join("workspace");
    let launch = match prepare_launch_config(&workspace) {
        Ok(launch) => launch,
        Err(error) => {
            return CodexProbeStatus::unavailable(format!(
                "Codex isolated runtime preparation failed: {error}"
            ))
        }
    };

    let candidates = discover_candidates();
    if candidates.is_empty() {
        return CodexProbeStatus::unavailable(format!(
            "Codex CLI {REQUIRED_CODEX_VERSION} native executable not found; set BSAIGC_CODEX_BIN"
        ));
    }
    let mut errors = Vec::new();
    for candidate in candidates {
        match probe_candidate(&candidate, &launch) {
            Ok(status) => return status,
            Err(error) => errors.push(format!("{}: {error}", candidate.source)),
        }
    }
    CodexProbeStatus::unavailable(format!(
        "Codex app-server handshake failed: {}",
        errors.join("; ")
    ))
}

pub(crate) fn prepare_launch_config(workspace_root: &Path) -> Result<CodexLaunchConfig, String> {
    let api_key = env::var("BSAIGC_CODEX_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let base_url = env::var("BSAIGC_CODEX_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let model = env::var("BSAIGC_CODEX_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    prepare_launch_config_with_provider(
        workspace_root,
        api_key.as_deref(),
        base_url.as_deref(),
        model.as_deref(),
    )
}

pub(crate) fn prepare_launch_config_with_api_key(
    workspace_root: &Path,
    api_key: Option<&str>,
) -> Result<CodexLaunchConfig, String> {
    let base_url = env::var("BSAIGC_CODEX_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let model = env::var("BSAIGC_CODEX_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    prepare_launch_config_with_provider(
        workspace_root,
        api_key,
        base_url.as_deref(),
        model.as_deref(),
    )
}

pub(crate) fn prepare_launch_config_with_provider(
    workspace_root: &Path,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: Option<&str>,
) -> Result<CodexLaunchConfig, String> {
    let base_url = validate_provider_base_url(base_url.unwrap_or(DEFAULT_PROVIDER_BASE_URL))?;
    let model = model
        .filter(|value| !value.trim().is_empty())
        .map(|value| validate_single_line("provider model", value, 128))
        .transpose()?;
    let working_directory = create_and_canonicalize(workspace_root, "brain workspace")?;
    if !working_directory.is_absolute() {
        return Err("brain workspace must resolve to an absolute directory".to_string());
    }
    let data_root = working_directory
        .parent()
        .ok_or("brain workspace has no parent data directory")?
        .to_path_buf();

    let requested_home = match env::var_os("BSAIGC_CODEX_HOME") {
        Some(value) if !value.is_empty() => {
            let value = PathBuf::from(value);
            if !value.is_absolute() {
                return Err("BSAIGC_CODEX_HOME must be absolute".to_string());
            }
            value
        }
        _ => data_root.join(MANAGED_HOME_DIR),
    };
    let codex_home = create_and_canonicalize(&requested_home, "managed Codex home")?;
    require_contained(&codex_home, &data_root, "managed Codex home")?;
    if codex_home == data_root || codex_home.starts_with(&working_directory) {
        return Err(
            "managed Codex home must be a sibling of the agent workspace, not writable by it"
                .to_string(),
        );
    }

    let profile = codex_home.join("profile");
    let roaming = profile.join("AppData").join("Roaming");
    let local = profile.join("AppData").join("Local");
    let temp = codex_home.join("tmp");
    for directory in [&profile, &roaming, &local, &temp] {
        fs::create_dir_all(directory)
            .map_err(|error| format!("create {} failed: {error}", directory.display()))?;
    }

    write_managed_config(&codex_home, &base_url, model.as_deref())?;
    let environment = managed_environment(&codex_home, &profile, &roaming, &local, &temp)?;
    Ok(CodexLaunchConfig {
        codex_home,
        working_directory,
        environment,
        api_key: api_key.map(|value| Zeroizing::new(value.to_string())),
    })
}

pub(crate) fn discover_candidates() -> Vec<Candidate> {
    let mut values = Vec::new();
    if let Some(override_path) = non_empty_env_os("BSAIGC_CODEX_BIN") {
        push_candidate(
            &mut values,
            PathBuf::from(override_path),
            "BSAIGC managed executable",
        );
    }

    if let Ok(current_exe) = env::current_exe() {
        if let Some(root) = current_exe.parent() {
            #[cfg(windows)]
            {
                push_candidate(
                    &mut values,
                    root.join("codex-runtime").join("codex.exe"),
                    "bundled runtime",
                );
                push_candidate(
                    &mut values,
                    root.join("resources")
                        .join("codex-runtime")
                        .join("codex.exe"),
                    "bundled resource runtime",
                );
            }
            #[cfg(not(windows))]
            {
                push_candidate(
                    &mut values,
                    root.join("codex-runtime").join("codex"),
                    "bundled runtime",
                );
                push_candidate(
                    &mut values,
                    root.join("resources").join("codex-runtime").join("codex"),
                    "bundled resource runtime",
                );
            }
        }
    }

    #[cfg(windows)]
    if let Some(app_data) = env::var_os("APPDATA") {
        let npm_root = PathBuf::from(app_data).join("npm");
        push_candidate(
            &mut values,
            npm_native_binary(&npm_root),
            "official npm runtime",
        );
    }

    values
}

fn push_candidate(values: &mut Vec<Candidate>, path: PathBuf, source: &str) {
    let Ok(path) = path.canonicalize() else {
        return;
    };
    if !is_allowed_native_executable(&path)
        || values
            .iter()
            .any(|existing| same_path(&existing.path, &path))
        || !candidate_version_matches(&path)
    {
        return;
    }
    values.push(Candidate {
        path,
        source: format!("{source} v{REQUIRED_CODEX_VERSION}"),
    });
}

#[cfg(windows)]
fn npm_native_binary(npm_root: &Path) -> PathBuf {
    let (package, target) = match env::consts::ARCH {
        "x86_64" => ("codex-win32-x64", "x86_64-pc-windows-msvc"),
        "aarch64" => ("codex-win32-arm64", "aarch64-pc-windows-msvc"),
        _ => return PathBuf::new(),
    };
    npm_root
        .join("node_modules")
        .join("@openai")
        .join("codex")
        .join("node_modules")
        .join("@openai")
        .join(package)
        .join("vendor")
        .join(target)
        .join("bin")
        .join("codex.exe")
}

fn is_allowed_native_executable(path: &Path) -> bool {
    if !path.is_absolute() || !path.is_file() {
        return false;
    }
    #[cfg(windows)]
    {
        let is_exe = path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));
        if !is_exe {
            return false;
        }
        let normalized = path.to_string_lossy().to_ascii_lowercase();
        if normalized.contains("\\windowsapps\\") {
            return false;
        }
    }
    true
}

fn candidate_version_matches(path: &Path) -> bool {
    let mut command = Command::new(path);
    command.arg("--version");
    command.env_clear();
    apply_base_os_environment(&mut command);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let Ok(output) = command.output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let version = String::from_utf8_lossy(&output.stdout);
    version.trim() == format!("codex-cli {REQUIRED_CODEX_VERSION}")
}

fn probe_candidate(
    candidate: &Candidate,
    launch: &CodexLaunchConfig,
) -> Result<CodexProbeStatus, String> {
    let mut command = app_server_command(&candidate.path, launch);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("app-server stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("app-server stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("app-server stderr unavailable")?;

    let (line_sender, line_receiver) = mpsc::channel::<String>();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if line_sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let (stderr_sender, stderr_receiver) = mpsc::channel::<String>();
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output);
        let _ = stderr_sender.send(output);
    });

    let initialize = json!({
        "id": 1,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "bsaigc-desktop",
                "title": "Banshan AIGC Desktop",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "experimentalApi": true,
                "requestAttestation": false
            }
        }
    });
    if let Err(error) = writeln!(stdin, "{initialize}").and_then(|_| stdin.flush()) {
        terminate(&mut child);
        return Err(format!("initialize write failed: {error}"));
    }

    let started = Instant::now();
    let initialize_result = loop {
        let Some(remaining) = HANDSHAKE_TIMEOUT.checked_sub(started.elapsed()) else {
            terminate(&mut child);
            return Err(with_stderr(
                "initialize response timed out",
                &stderr_receiver,
            ));
        };
        let line = match line_receiver.recv_timeout(remaining) {
            Ok(line) => line,
            Err(_) => {
                terminate(&mut child);
                return Err(with_stderr(
                    "initialize response timed out",
                    &stderr_receiver,
                ));
            }
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if message.get("id") == Some(&json!(1)) {
            if let Some(error) = message.get("error") {
                terminate(&mut child);
                return Err(format!("initialize rejected: {error}"));
            }
            let Some(result) = message.get("result").cloned() else {
                terminate(&mut child);
                return Err("initialize response did not contain result".to_string());
            };
            break result;
        }
    };

    if let Err(error) =
        writeln!(stdin, "{}", json!({ "method": "initialized" })).and_then(|_| stdin.flush())
    {
        terminate(&mut child);
        return Err(format!("initialized write failed: {error}"));
    }
    drop(stdin);
    terminate(&mut child);

    let user_agent = initialize_result
        .get("userAgent")
        .and_then(Value::as_str)
        .map(str::to_string);
    if user_agent.is_none() {
        return Err(with_stderr(
            "initialize result has no userAgent",
            &stderr_receiver,
        ));
    }
    let returned_home = initialize_result
        .get("codexHome")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let codex_home_ready = returned_home
        .as_deref()
        .is_some_and(|value| same_path(value, &launch.codex_home));
    if !codex_home_ready {
        return Err("app-server did not use the BSAIGC-managed CODEX_HOME".to_string());
    }

    Ok(CodexProbeStatus {
        available: true,
        runtime: "official-codex-app-server".to_string(),
        transport: "stdio/jsonl".to_string(),
        user_agent,
        platform_family: initialize_result
            .get("platformFamily")
            .and_then(Value::as_str)
            .map(str::to_string),
        platform_os: initialize_result
            .get("platformOs")
            .and_then(Value::as_str)
            .map(str::to_string),
        codex_home_ready,
        source: Some(candidate.source.clone()),
        handshake_at: Some(now_millis()),
        error: None,
    })
}

pub(crate) fn app_server_command(path: &Path, launch: &CodexLaunchConfig) -> Command {
    let mut command = Command::new(path);
    command.arg("app-server").arg("--listen").arg("stdio://");
    command.current_dir(&launch.working_directory);
    command.env_clear();
    for (name, value) in &launch.environment {
        command.env(name, value);
    }
    if let Some(api_key) = launch.api_key.as_ref() {
        command.env("BSAIGC_CODEX_API_KEY", api_key.as_str());
    }
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn write_managed_config(
    codex_home: &Path,
    base_url: &str,
    model: Option<&str>,
) -> Result<(), String> {
    let config = managed_config(base_url, model);
    let path = codex_home.join("config.toml");
    if fs::read_to_string(&path).ok().as_deref() != Some(config.as_str()) {
        fs::write(&path, config)
            .map_err(|error| format!("write managed Codex config failed: {error}"))?;
    }
    Ok(())
}

fn managed_config(base_url: &str, model: Option<&str>) -> String {
    let model = model
        .map(|value| format!("model = {}\n", toml_string(value)))
        .unwrap_or_default();
    format!(
        "# Managed by BSAIGC Desktop. Personal Codex config, plugins, MCP, sessions, and auth are intentionally not inherited.\n\
model_provider = \"{PRODUCT_PROVIDER_ID}\"\n\
{model}\
approval_policy = \"on-request\"\n\
sandbox_mode = \"workspace-write\"\n\
project_doc_max_bytes = 0\n\
\n\
[model_providers.{PRODUCT_PROVIDER_ID}]\n\
name = \"BSAIGC\"\n\
base_url = {}\n\
env_key = \"BSAIGC_CODEX_API_KEY\"\n\
wire_api = \"responses\"\n\
requires_openai_auth = false\n\
\n\
[shell_environment_policy]\n\
inherit = \"core\"\n",
        toml_string(base_url)
    )
}

fn managed_environment(
    codex_home: &Path,
    profile: &Path,
    roaming: &Path,
    local: &Path,
    temp: &Path,
) -> Result<Vec<(OsString, OsString)>, String> {
    let mut values = Vec::new();
    push_env(&mut values, "CODEX_HOME", codex_home.as_os_str());
    push_env(&mut values, "HOME", profile.as_os_str());
    push_env(&mut values, "USERPROFILE", profile.as_os_str());
    push_env(&mut values, "APPDATA", roaming.as_os_str());
    push_env(&mut values, "LOCALAPPDATA", local.as_os_str());
    push_env(&mut values, "TEMP", temp.as_os_str());
    push_env(&mut values, "TMP", temp.as_os_str());

    append_base_os_environment(&mut values)?;

    copy_managed_env(
        &mut values,
        "BSAIGC_CODEX_OPENAI_ORGANIZATION",
        "OPENAI_ORGANIZATION",
    );
    copy_managed_env(&mut values, "BSAIGC_CODEX_OPENAI_PROJECT", "OPENAI_PROJECT");
    copy_proxy_env(&mut values, "BSAIGC_CODEX_HTTP_PROXY", "HTTP_PROXY");
    copy_proxy_env(&mut values, "BSAIGC_CODEX_HTTPS_PROXY", "HTTPS_PROXY");
    copy_proxy_env(&mut values, "BSAIGC_CODEX_ALL_PROXY", "ALL_PROXY");
    copy_proxy_env(&mut values, "BSAIGC_CODEX_NO_PROXY", "NO_PROXY");
    copy_managed_env(&mut values, "BSAIGC_CODEX_SSL_CERT_FILE", "SSL_CERT_FILE");

    Ok(values)
}

fn append_base_os_environment(values: &mut Vec<(OsString, OsString)>) -> Result<(), String> {
    #[cfg(windows)]
    {
        let system_root = env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let comspec = system_root.join("System32").join("cmd.exe");
        push_env(values, "SystemRoot", system_root.as_os_str());
        push_env(values, "WINDIR", system_root.as_os_str());
        push_env(values, "COMSPEC", comspec.as_os_str());
        push_env(values, "PATHEXT", OsStr::new(".COM;.EXE;.BAT;.CMD"));
        push_env(values, "OS", OsStr::new("Windows_NT"));
        for name in [
            "SystemDrive",
            "PROCESSOR_ARCHITECTURE",
            "PROCESSOR_IDENTIFIER",
            "NUMBER_OF_PROCESSORS",
        ] {
            if let Some(value) = non_empty_env_os(name) {
                push_env(values, name, &value);
            }
        }
        let path = managed_windows_path(&system_root)?;
        push_env(values, "PATH", &path);
    }
    #[cfg(not(windows))]
    {
        push_env(values, "PATH", OsStr::new("/usr/local/bin:/usr/bin:/bin"));
        for name in ["LANG", "LC_ALL"] {
            if let Some(value) = non_empty_env_os(name) {
                push_env(values, name, &value);
            }
        }
    }
    Ok(())
}

fn apply_base_os_environment(command: &mut Command) {
    let mut values = Vec::new();
    if append_base_os_environment(&mut values).is_ok() {
        command.envs(values);
    }
}

#[cfg(windows)]
fn managed_windows_path(system_root: &Path) -> Result<OsString, String> {
    let mut paths = vec![
        system_root.join("System32"),
        system_root.to_path_buf(),
        system_root.join("System32").join("Wbem"),
        system_root
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0"),
    ];
    if let Some(program_files) = env::var_os("ProgramFiles") {
        let program_files = PathBuf::from(program_files);
        paths.push(program_files.join("Git").join("cmd"));
        paths.push(program_files.join("PowerShell").join("7"));
        paths.push(program_files.join("nodejs"));
    }
    if let Some(extra) = non_empty_env_os("BSAIGC_CODEX_PATH") {
        for path in env::split_paths(&extra) {
            if !path.is_absolute() || !path.is_dir() || is_forbidden_path_entry(&path) {
                continue;
            }
            paths.push(path);
        }
    }

    let mut seen = HashSet::new();
    paths.retain(|path| {
        path.is_dir()
            && !is_forbidden_path_entry(path)
            && seen.insert(path.to_string_lossy().to_ascii_lowercase())
    });
    env::join_paths(paths).map_err(|error| format!("build managed PATH failed: {error}"))
}

#[cfg(windows)]
fn is_forbidden_path_entry(path: &Path) -> bool {
    let normalized = path.to_string_lossy().to_ascii_lowercase();
    normalized.contains("\\windowsapps\\")
        || normalized.ends_with("\\windowsapps")
        || normalized.contains("\\openai\\codex\\")
        || normalized.contains("\\.codex\\")
}

fn copy_managed_env(values: &mut Vec<(OsString, OsString)>, source: &str, destination: &str) {
    if let Some(value) = non_empty_env_os(source) {
        push_env(values, destination, &value);
    }
}

fn copy_proxy_env(values: &mut Vec<(OsString, OsString)>, source: &str, destination: &str) {
    if let Some(value) = non_empty_env_os(source) {
        push_env(values, destination, &value);
        push_env(values, &destination.to_ascii_lowercase(), &value);
    }
}

fn push_env(values: &mut Vec<(OsString, OsString)>, name: &str, value: &OsStr) {
    values.push((OsString::from(name), value.to_os_string()));
}

fn non_empty_env_os(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}

fn create_and_canonicalize(path: &Path, label: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(path).map_err(|error| format!("create {label} failed: {error}"))?;
    path.canonicalize()
        .map_err(|error| format!("resolve {label} failed: {error}"))
}

fn require_contained(path: &Path, root: &Path, label: &str) -> Result<(), String> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(format!("{label} escapes the BSAIGC data root"))
    }
}

fn validate_provider_base_url(value: &str) -> Result<String, String> {
    let value = validate_single_line("BSAIGC_CODEX_BASE_URL", value, 2_048)?;
    if value.chars().any(char::is_whitespace) || value.contains('?') || value.contains('#') {
        return Err(
            "BSAIGC_CODEX_BASE_URL must not contain whitespace, query, or fragment data"
                .to_string(),
        );
    }

    let (scheme, remainder) = value
        .split_once("://")
        .ok_or("BSAIGC_CODEX_BASE_URL must be an absolute HTTP(S) URL")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        return Err("BSAIGC_CODEX_BASE_URL must use HTTP or HTTPS".to_string());
    }

    let authority = remainder.split('/').next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return Err("BSAIGC_CODEX_BASE_URL must contain a host and no credentials".to_string());
    }
    let host = provider_authority_host(authority)?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false);
    if scheme == "http" && !loopback {
        return Err(
            "BSAIGC_CODEX_BASE_URL must use HTTPS (plain HTTP is allowed only for loopback)"
                .to_string(),
        );
    }

    Ok(value.trim_end_matches('/').to_string())
}

fn provider_authority_host(authority: &str) -> Result<&str, String> {
    if let Some(bracketed) = authority.strip_prefix('[') {
        let end = bracketed
            .find(']')
            .ok_or("BSAIGC_CODEX_BASE_URL contains an invalid bracketed host")?;
        let host = &bracketed[..end];
        if host.parse::<IpAddr>().is_err() {
            return Err("BSAIGC_CODEX_BASE_URL contains an invalid IP address".to_string());
        }
        validate_provider_port(&bracketed[end + 1..])?;
        return Ok(host);
    }

    if authority.matches(':').count() > 1 {
        return Err("BSAIGC_CODEX_BASE_URL must bracket IPv6 addresses".to_string());
    }
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, ""), |(host, port)| (host, port));
    if host.is_empty() {
        return Err("BSAIGC_CODEX_BASE_URL must contain a host".to_string());
    }
    if authority.contains(':') {
        validate_provider_port(&format!(":{port}"))?;
    }
    Ok(host)
}

fn validate_provider_port(suffix: &str) -> Result<(), String> {
    if suffix.is_empty() {
        return Ok(());
    }
    let port = suffix
        .strip_prefix(':')
        .ok_or("BSAIGC_CODEX_BASE_URL contains invalid data after the host")?;
    if port.is_empty() || port.parse::<u16>().is_err() {
        return Err("BSAIGC_CODEX_BASE_URL contains an invalid port".to_string());
    }
    Ok(())
}
fn validate_single_line(name: &str, value: &str, max_chars: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars {
        return Err(format!("{name} is empty or too long"));
    }
    if value
        .chars()
        .any(|character| matches!(character, '\r' | '\n' | '\0'))
    {
        return Err(format!("{name} must be a single line"));
    }
    Ok(value.to_string())
}

fn toml_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        normalize_windows_path(left) == normalize_windows_path(right)
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

#[cfg(windows)]
fn normalize_windows_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    let value = if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = value.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        value.into_owned()
    };
    value.trim_end_matches(['\\', '/']).to_ascii_lowercase()
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn with_stderr(message: &str, receiver: &mpsc::Receiver<String>) -> String {
    let stderr = receiver
        .recv_timeout(Duration::from_millis(500))
        .unwrap_or_default();
    let compact = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" | ");
    if !compact.is_empty() {
        eprintln!("codex app-server diagnostic: {compact}");
    }
    message.to_string()
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_config_isolated_from_desktop_private_state() {
        let config = managed_config("https://api.example.test/v1", Some("gpt-test"));
        assert!(config.contains("model_provider = \"bsaigc\""));
        assert!(config.contains("env_key = \"BSAIGC_CODEX_API_KEY\""));
        assert!(config.contains("project_doc_max_bytes = 0"));
        assert!(config.contains("inherit = \"core\""));
        assert!(!config.contains("disable_response_storage"));
        assert!(!config.contains("mcp_servers"));
        assert!(!config.contains("plugins."));
        assert!(!config.contains("notify"));
    }

    #[test]
    fn explicit_provider_key_is_redacted_and_only_attached_to_child_environment() {
        let root = tempfile::tempdir().unwrap();
        let secret = "sk-test-child-environment-only";
        let launch =
            prepare_launch_config_with_api_key(&root.path().join("brain-workspace"), Some(secret))
                .unwrap();

        assert!(!format!("{launch:?}").contains(secret));
        assert!(!launch
            .environment
            .iter()
            .any(|(name, _)| { name.to_string_lossy() == "BSAIGC_CODEX_API_KEY" }));

        let command = app_server_command(Path::new("codex.exe"), &launch);
        let injected = command
            .get_envs()
            .find(|(name, _)| name.to_string_lossy() == "BSAIGC_CODEX_API_KEY")
            .and_then(|(_, value)| value)
            .map(|value| value.to_string_lossy().into_owned());
        assert_eq!(injected.as_deref(), Some(secret));
    }

    #[test]
    fn provider_url_rejects_credentials_and_remote_plain_http() {
        assert!(validate_provider_base_url("https://api.example.test/v1").is_ok());
        assert!(validate_provider_base_url("http://127.0.0.1:8080/v1").is_ok());
        assert!(validate_provider_base_url("http://api.example.test/v1").is_err());
        assert!(validate_provider_base_url("http://localhost.evil.test/v1").is_err());
        assert!(validate_provider_base_url("http://127.0.0.1.evil.test/v1").is_err());
        assert!(validate_provider_base_url("https://token@api.example.test/v1").is_err());
        assert!(validate_provider_base_url("https://api.example.test/v1?token=x").is_err());
        assert!(validate_provider_base_url("https:///v1").is_err());
    }

    #[test]
    fn containment_rejects_sibling_escape() {
        let root = tempfile::tempdir().unwrap();
        let allowed = root.path().join("allowed");
        let inside = allowed.join("inside");
        let outside = root.path().join("outside");
        fs::create_dir_all(&inside).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let allowed = allowed.canonicalize().unwrap();
        assert!(require_contained(&inside.canonicalize().unwrap(), &allowed, "inside").is_ok());
        assert!(require_contained(&outside.canonicalize().unwrap(), &allowed, "outside").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn extended_windows_paths_compare_equal_to_dos_paths() {
        assert!(same_path(
            Path::new(r"\\?\C:\Users\operator\codex-home"),
            Path::new(r"C:\Users\operator\codex-home\")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windowsapps_and_personal_codex_paths_are_forbidden() {
        assert!(is_forbidden_path_entry(Path::new(
            r"C:\Users\operator\AppData\Local\Microsoft\WindowsApps"
        )));
        assert!(is_forbidden_path_entry(Path::new(
            r"C:\Users\operator\.codex\bin"
        )));
        assert!(is_forbidden_path_entry(Path::new(
            r"C:\Users\operator\AppData\Local\OpenAI\Codex\bin"
        )));
        assert!(!is_forbidden_path_entry(Path::new(r"C:\Windows\System32")));
    }

    #[test]
    #[ignore = "requires the pinned official Codex CLI"]
    fn official_app_server_handshake() {
        let status = probe_codex();
        assert!(status.available, "{:?}", status.error);
        assert!(status.user_agent.is_some());
        assert!(status.codex_home_ready);
    }
}
