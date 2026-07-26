use crate::codex_host::{
    app_server_command, discover_candidates, prepare_launch_config,
    prepare_launch_config_with_api_key, prepare_launch_config_with_provider, Candidate,
    CodexLaunchConfig,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);
const REDACTED_PATH: &str = "[redacted-local-path]";
pub const DYNAMIC_TOOL_CALL_METHOD: &str = "item/tool/call";

type PendingSender = mpsc::Sender<Result<Value, CodexRuntimeError>>;
type NotificationCallback = Arc<dyn Fn(CodexNotification) + Send + Sync + 'static>;

/// Lightweight cooperative cancellation shared by durable backend stages.
/// It intentionally has no async-runtime dependency so Rust/FFmpeg/OCR/Codex
/// workers can all observe the same signal.
#[derive(Debug, Clone, Default)]
pub(crate) struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn check_cancelled(&self) -> Result<(), crate::protocol::HostError> {
        if self.is_cancelled() {
            Err(crate::protocol::HostError::new(
                "CONTRACT_REVIEW_CANCELLED",
                "contract review was cancelled",
                false,
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CodexRuntimeError {
    Unavailable(String),
    SpawnFailed,
    StdioUnavailable(&'static str),
    NotInitialized,
    ShuttingDown,
    ProcessExited(String),
    DeadlineExceeded {
        method: String,
    },
    Transport(String),
    Protocol(String),
    Remote {
        code: Option<i64>,
        message: String,
        data: Option<Value>,
    },
}

impl fmt::Display for CodexRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) => write!(formatter, "Codex runtime unavailable: {message}"),
            Self::SpawnFailed => write!(formatter, "Codex app-server could not be started"),
            Self::StdioUnavailable(stream) => {
                write!(formatter, "Codex app-server {stream} is unavailable")
            }
            Self::NotInitialized => write!(formatter, "Codex runtime is not initialized"),
            Self::ShuttingDown => write!(formatter, "Codex runtime is shutting down"),
            Self::ProcessExited(reason) => write!(formatter, "Codex app-server exited: {reason}"),
            Self::DeadlineExceeded { method } => {
                write!(formatter, "Codex request deadline exceeded: {method}")
            }
            Self::Transport(message) => write!(formatter, "Codex transport failed: {message}"),
            Self::Protocol(message) => write!(formatter, "Codex protocol failed: {message}"),
            Self::Remote { code, message, .. } => match code {
                Some(code) => write!(formatter, "Codex request rejected ({code}): {message}"),
                None => write!(formatter, "Codex request rejected: {message}"),
            },
        }
    }
}

impl std::error::Error for CodexRuntimeError {}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexNotification {
    pub request_id: Option<Value>,
    pub method: String,
    pub params: Value,
    pub received_at: u64,
}

impl CodexNotification {
    pub fn parsed_request_id(&self) -> Result<Option<CodexRequestId>, CodexRuntimeError> {
        self.request_id
            .clone()
            .map(|value| {
                serde_json::from_value(value).map_err(|error| {
                    CodexRuntimeError::Protocol(format!("invalid app-server request id: {error}"))
                })
            })
            .transpose()
    }

    pub fn dynamic_tool_call(&self) -> Result<Option<DynamicToolCallRequest>, CodexRuntimeError> {
        if self.method != DYNAMIC_TOOL_CALL_METHOD {
            return Ok(None);
        }
        let request_id = self.parsed_request_id()?.ok_or_else(|| {
            CodexRuntimeError::Protocol("item/tool/call request did not include an id".to_string())
        })?;
        let params = serde_json::from_value(self.params.clone()).map_err(|error| {
            CodexRuntimeError::Protocol(format!("invalid item/tool/call params: {error}"))
        })?;
        Ok(Some(DynamicToolCallRequest { request_id, params }))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexRuntimeHealth {
    pub running: bool,
    pub initialized: bool,
    pub pending_requests: usize,
    pub subscribers: usize,
    pub source: String,
    pub user_agent: Option<String>,
    pub platform_family: Option<String>,
    pub platform_os: Option<String>,
    pub started_at: u64,
    pub last_message_at: Option<u64>,
    pub exit_reason: Option<String>,
}

pub struct CodexSubscription {
    id: u64,
    subscribers: Weak<Mutex<HashMap<u64, NotificationCallback>>>,
}

impl Drop for CodexSubscription {
    fn drop(&mut self) {
        if let Some(subscribers) = self.subscribers.upgrade() {
            lock_unpoisoned(&subscribers).remove(&self.id);
        }
    }
}

struct RuntimeShared {
    pending: Mutex<HashMap<u64, PendingSender>>,
    subscribers: Arc<Mutex<HashMap<u64, NotificationCallback>>>,
    child: Mutex<Option<Child>>,
    running: AtomicBool,
    initialized: AtomicBool,
    shutting_down: AtomicBool,
    exit_reason: Mutex<Option<String>>,
    last_message_at: AtomicU64,
}

struct RuntimeInner {
    shared: Arc<RuntimeShared>,
    outbound: Mutex<Option<mpsc::Sender<String>>>,
    next_request_id: AtomicU64,
    next_subscription_id: AtomicU64,
    source: String,
    user_agent: Option<String>,
    platform_family: Option<String>,
    platform_os: Option<String>,
    started_at: u64,
    reader_join: Mutex<Option<JoinHandle<()>>>,
    writer_join: Mutex<Option<JoinHandle<()>>>,
    stderr_join: Mutex<Option<JoinHandle<()>>>,
    notification_join: Mutex<Option<JoinHandle<()>>>,
    notification_sender: Mutex<Option<mpsc::Sender<CodexNotification>>>,
}

#[derive(Clone)]
pub struct CodexRuntime {
    inner: Arc<RuntimeInner>,
}

fn initialize_params() -> Value {
    json!({
        "clientInfo": {
            "name": "bsaigc-desktop",
            "title": "Banshan AIGC Desktop",
            "version": env!("CARGO_PKG_VERSION")
        },
        "capabilities": {
            "experimentalApi": true,
            "requestAttestation": false
        }
    })
}

impl CodexRuntime {
    pub fn start(workspace_root: &Path) -> Result<Self, CodexRuntimeError> {
        let launch =
            prepare_launch_config(workspace_root).map_err(CodexRuntimeError::Unavailable)?;
        Self::start_with_launch(&launch)
    }

    pub fn start_with_api_key(
        workspace_root: &Path,
        api_key: Option<&str>,
    ) -> Result<Self, CodexRuntimeError> {
        let launch = prepare_launch_config_with_api_key(workspace_root, api_key)
            .map_err(CodexRuntimeError::Unavailable)?;
        Self::start_with_launch(&launch)
    }

    pub fn start_with_provider(
        workspace_root: &Path,
        api_key: Option<&str>,
        base_url: Option<&str>,
        model: Option<&str>,
    ) -> Result<Self, CodexRuntimeError> {
        let launch = prepare_launch_config_with_provider(workspace_root, api_key, base_url, model)
            .map_err(CodexRuntimeError::Unavailable)?;
        Self::start_with_launch(&launch)
    }

    fn start_with_launch(launch: &CodexLaunchConfig) -> Result<Self, CodexRuntimeError> {
        let candidates = discover_candidates();
        if candidates.is_empty() {
            return Err(CodexRuntimeError::Unavailable(format!(
                "pinned Codex CLI {} native executable was not found",
                crate::codex_host::REQUIRED_CODEX_VERSION
            )));
        }

        let mut errors = Vec::new();
        for candidate in candidates {
            match Self::start_candidate(&candidate, launch) {
                Ok(runtime) => return Ok(runtime),
                Err(error) => errors.push(format!("{}: {error}", candidate.source)),
            }
        }
        Err(CodexRuntimeError::Unavailable(errors.join("; ")))
    }

    fn start_candidate(
        candidate: &Candidate,
        launch: &CodexLaunchConfig,
    ) -> Result<Self, CodexRuntimeError> {
        let mut command = app_server_command(&candidate.path, launch);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            eprintln!("codex app-server spawn diagnostic: {error}");
            CodexRuntimeError::SpawnFailed
        })?;

        let stdin = take_stdin(&mut child)?;
        let stdout = take_stdout(&mut child)?;
        let stderr = take_stderr(&mut child)?;
        let subscribers = Arc::new(Mutex::new(HashMap::new()));
        let shared = Arc::new(RuntimeShared {
            pending: Mutex::new(HashMap::new()),
            subscribers: Arc::clone(&subscribers),
            child: Mutex::new(Some(child)),
            running: AtomicBool::new(true),
            initialized: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
            exit_reason: Mutex::new(None),
            last_message_at: AtomicU64::new(0),
        });

        let (outbound_sender, outbound_receiver) = mpsc::channel();
        let (notification_sender, notification_receiver) = mpsc::channel();
        let writer_join = spawn_writer(stdin, outbound_receiver, Arc::clone(&shared))?;
        let reader_join =
            match spawn_reader(stdout, Arc::clone(&shared), notification_sender.clone()) {
                Ok(handle) => handle,
                Err(error) => {
                    drop(outbound_sender);
                    terminate_shared_child(&shared);
                    join_worker(writer_join);
                    return Err(error);
                }
            };
        let stderr_join = match spawn_stderr_drain(stderr) {
            Ok(handle) => handle,
            Err(error) => {
                drop(outbound_sender);
                terminate_shared_child(&shared);
                join_worker(writer_join);
                join_worker(reader_join);
                return Err(error);
            }
        };
        let notification_join =
            match spawn_notification_dispatch(notification_receiver, Arc::clone(&subscribers)) {
                Ok(handle) => handle,
                Err(error) => {
                    drop(outbound_sender);
                    drop(notification_sender);
                    terminate_shared_child(&shared);
                    join_worker(writer_join);
                    join_worker(reader_join);
                    join_worker(stderr_join);
                    return Err(error);
                }
            };

        let inner = Arc::new(RuntimeInner {
            shared,
            outbound: Mutex::new(Some(outbound_sender)),
            next_request_id: AtomicU64::new(1),
            next_subscription_id: AtomicU64::new(1),
            source: candidate.source.clone(),
            user_agent: None,
            platform_family: None,
            platform_os: None,
            started_at: now_millis(),
            reader_join: Mutex::new(Some(reader_join)),
            writer_join: Mutex::new(Some(writer_join)),
            stderr_join: Mutex::new(Some(stderr_join)),
            notification_join: Mutex::new(Some(notification_join)),
            notification_sender: Mutex::new(Some(notification_sender)),
        });
        let mut runtime = Self { inner };
        let initialize = initialize_params();
        let deadline = Instant::now() + INITIALIZE_TIMEOUT;
        let result = match runtime.request_internal("initialize", initialize, deadline, false) {
            Ok(result) => result,
            Err(error) => {
                runtime.shutdown();
                return Err(error);
            }
        };
        if let Err(error) = runtime.notify_internal("initialized", None, false) {
            runtime.shutdown();
            return Err(error);
        }
        runtime
            .inner
            .shared
            .initialized
            .store(true, Ordering::Release);

        if let Some(inner) = Arc::get_mut(&mut runtime.inner) {
            inner.user_agent = result
                .get("userAgent")
                .and_then(Value::as_str)
                .map(str::to_string);
            inner.platform_family = result
                .get("platformFamily")
                .and_then(Value::as_str)
                .map(str::to_string);
            inner.platform_os = result
                .get("platformOs")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        if runtime.inner.user_agent.is_none() {
            runtime.shutdown();
            return Err(CodexRuntimeError::Protocol(
                "initialize result did not include userAgent".to_string(),
            ));
        }
        Ok(runtime)
    }

    pub fn request(
        &self,
        method: &str,
        params: Value,
        deadline: Instant,
    ) -> Result<Value, CodexRuntimeError> {
        self.request_internal(method, params, deadline, true)
    }

    fn request_internal(
        &self,
        method: &str,
        params: Value,
        deadline: Instant,
        require_initialized: bool,
    ) -> Result<Value, CodexRuntimeError> {
        if method.trim().is_empty() {
            return Err(CodexRuntimeError::Protocol(
                "request method cannot be empty".to_string(),
            ));
        }
        self.ensure_available(require_initialized)?;
        if deadline <= Instant::now() {
            return Err(CodexRuntimeError::DeadlineExceeded {
                method: method.to_string(),
            });
        }

        let id = self.inner.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (response_sender, response_receiver) = mpsc::channel();
        lock_unpoisoned(&self.inner.shared.pending).insert(id, response_sender);
        let message = json!({ "id": id, "method": method, "params": params }).to_string();
        if let Err(error) = self.send_line(message) {
            lock_unpoisoned(&self.inner.shared.pending).remove(&id);
            return Err(error);
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        match response_receiver.recv_timeout(remaining) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                lock_unpoisoned(&self.inner.shared.pending).remove(&id);
                match response_receiver.try_recv() {
                    Ok(result) => result,
                    Err(_) => Err(CodexRuntimeError::DeadlineExceeded {
                        method: method.to_string(),
                    }),
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(CodexRuntimeError::ProcessExited(
                current_exit_reason(&self.inner.shared),
            )),
        }
    }

    pub fn notify(&self, method: &str, params: Option<Value>) -> Result<(), CodexRuntimeError> {
        self.notify_internal(method, params, true)
    }

    /// Sends a successful response to an app-server initiated request.
    /// Codex app-server intentionally uses JSON-RPC-shaped messages without a
    /// `jsonrpc` member, so this must not go through a generic JSON-RPC client.
    pub fn respond(
        &self,
        request_id: CodexRequestId,
        result: Value,
    ) -> Result<(), CodexRuntimeError> {
        self.ensure_available(true)?;
        self.send_line(json!({ "id": request_id, "result": result }).to_string())
    }

    pub fn respond_dynamic_tool(
        &self,
        request_id: CodexRequestId,
        response: DynamicToolCallResponse,
    ) -> Result<(), CodexRuntimeError> {
        let result = serde_json::to_value(response)
            .map_err(|error| CodexRuntimeError::Protocol(error.to_string()))?;
        self.respond(request_id, result)
    }

    fn notify_internal(
        &self,
        method: &str,
        params: Option<Value>,
        require_initialized: bool,
    ) -> Result<(), CodexRuntimeError> {
        if method.trim().is_empty() {
            return Err(CodexRuntimeError::Protocol(
                "notification method cannot be empty".to_string(),
            ));
        }
        self.ensure_available(require_initialized)?;
        let mut message = Map::new();
        message.insert("method".to_string(), Value::String(method.to_string()));
        if let Some(params) = params {
            message.insert("params".to_string(), params);
        }
        self.send_line(Value::Object(message).to_string())
    }

    pub fn subscribe<F>(&self, callback: F) -> CodexSubscription
    where
        F: Fn(CodexNotification) + Send + Sync + 'static,
    {
        let id = self
            .inner
            .next_subscription_id
            .fetch_add(1, Ordering::Relaxed);
        lock_unpoisoned(&self.inner.shared.subscribers).insert(id, Arc::new(callback));
        CodexSubscription {
            id,
            subscribers: Arc::downgrade(&self.inner.shared.subscribers),
        }
    }

    pub fn health(&self) -> CodexRuntimeHealth {
        refresh_process_status(&self.inner.shared);
        CodexRuntimeHealth {
            running: self.inner.shared.running.load(Ordering::Acquire),
            initialized: self.inner.shared.initialized.load(Ordering::Acquire),
            pending_requests: lock_unpoisoned(&self.inner.shared.pending).len(),
            subscribers: lock_unpoisoned(&self.inner.shared.subscribers).len(),
            source: self.inner.source.clone(),
            user_agent: self.inner.user_agent.clone(),
            platform_family: self.inner.platform_family.clone(),
            platform_os: self.inner.platform_os.clone(),
            started_at: self.inner.started_at,
            last_message_at: nonzero(self.inner.shared.last_message_at.load(Ordering::Acquire)),
            exit_reason: lock_unpoisoned(&self.inner.shared.exit_reason).clone(),
        }
    }

    pub fn shutdown(&self) {
        shutdown_inner(&self.inner);
    }

    pub fn thread_start(
        &self,
        params: ThreadStartParams,
        deadline: Instant,
    ) -> Result<Value, CodexRuntimeError> {
        self.request_typed("thread/start", params, deadline)
    }

    pub fn thread_resume(
        &self,
        params: ThreadResumeParams,
        deadline: Instant,
    ) -> Result<Value, CodexRuntimeError> {
        self.request_typed("thread/resume", params, deadline)
    }

    pub fn thread_list(
        &self,
        params: ThreadListParams,
        deadline: Instant,
    ) -> Result<Value, CodexRuntimeError> {
        self.request_typed("thread/list", params, deadline)
    }

    pub fn turn_start(
        &self,
        params: TurnStartParams,
        deadline: Instant,
    ) -> Result<Value, CodexRuntimeError> {
        self.request_typed("turn/start", params, deadline)
    }

    pub fn turn_interrupt(
        &self,
        params: TurnInterruptParams,
        deadline: Instant,
    ) -> Result<Value, CodexRuntimeError> {
        self.request_typed("turn/interrupt", params, deadline)
    }

    fn request_typed<P: Serialize>(
        &self,
        method: &str,
        params: P,
        deadline: Instant,
    ) -> Result<Value, CodexRuntimeError> {
        let params = serde_json::to_value(params)
            .map_err(|error| CodexRuntimeError::Protocol(error.to_string()))?;
        self.request(method, params, deadline)
    }

    fn ensure_available(&self, require_initialized: bool) -> Result<(), CodexRuntimeError> {
        if self.inner.shared.shutting_down.load(Ordering::Acquire) {
            return Err(CodexRuntimeError::ShuttingDown);
        }
        if !self.inner.shared.running.load(Ordering::Acquire) {
            return Err(CodexRuntimeError::ProcessExited(current_exit_reason(
                &self.inner.shared,
            )));
        }
        if require_initialized && !self.inner.shared.initialized.load(Ordering::Acquire) {
            return Err(CodexRuntimeError::NotInitialized);
        }
        Ok(())
    }

    fn send_line(&self, message: String) -> Result<(), CodexRuntimeError> {
        let sender = lock_unpoisoned(&self.inner.outbound).clone();
        let Some(sender) = sender else {
            return Err(CodexRuntimeError::ShuttingDown);
        };
        sender.send(message).map_err(|_| {
            mark_dead(
                &self.inner.shared,
                CodexRuntimeError::ProcessExited("writer channel closed".to_string()),
            );
            CodexRuntimeError::ProcessExited("writer channel closed".to_string())
        })
    }
}

impl Drop for RuntimeInner {
    fn drop(&mut self) {
        shutdown_inner(self);
    }
}

fn spawn_writer(
    stdin: ChildStdin,
    receiver: mpsc::Receiver<String>,
    shared: Arc<RuntimeShared>,
) -> Result<JoinHandle<()>, CodexRuntimeError> {
    thread::Builder::new()
        .name("bsaigc-codex-writer".to_string())
        .spawn(move || writer_loop(stdin, receiver, shared))
        .map_err(|error| CodexRuntimeError::Transport(format!("writer thread: {error}")))
}

fn writer_loop(
    mut stdin: ChildStdin,
    receiver: mpsc::Receiver<String>,
    shared: Arc<RuntimeShared>,
) {
    while let Ok(line) = receiver.recv() {
        if writeln!(stdin, "{line}")
            .and_then(|_| stdin.flush())
            .is_err()
        {
            mark_dead(
                &shared,
                CodexRuntimeError::ProcessExited("stdin write failed".to_string()),
            );
            terminate_shared_child(&shared);
            break;
        }
    }
}

fn spawn_reader(
    stdout: ChildStdout,
    shared: Arc<RuntimeShared>,
    notification_sender: mpsc::Sender<CodexNotification>,
) -> Result<JoinHandle<()>, CodexRuntimeError> {
    thread::Builder::new()
        .name("bsaigc-codex-reader".to_string())
        .spawn(move || reader_loop(stdout, shared, notification_sender))
        .map_err(|error| CodexRuntimeError::Transport(format!("reader thread: {error}")))
}

fn reader_loop(
    stdout: ChildStdout,
    shared: Arc<RuntimeShared>,
    notification_sender: mpsc::Sender<CodexNotification>,
) {
    for line in BufReader::new(stdout).lines() {
        match line {
            Ok(line) => match serde_json::from_str::<Value>(&line) {
                Ok(message) => route_message(&shared, Some(&notification_sender), message),
                Err(error) => eprintln!("codex app-server invalid JSONL diagnostic: {error}"),
            },
            Err(error) => {
                eprintln!("codex app-server stdout diagnostic: {error}");
                break;
            }
        }
    }
    let failure = if shared.shutting_down.load(Ordering::Acquire) {
        CodexRuntimeError::ShuttingDown
    } else {
        CodexRuntimeError::ProcessExited("stdout closed".to_string())
    };
    mark_dead(&shared, failure);
    terminate_shared_child(&shared);
}

fn spawn_stderr_drain(stderr: ChildStderr) -> Result<JoinHandle<()>, CodexRuntimeError> {
    thread::Builder::new()
        .name("bsaigc-codex-stderr".to_string())
        .spawn(move || {
            for line in BufReader::new(stderr).lines() {
                match line {
                    Ok(line) if !line.trim().is_empty() => {
                        eprintln!("codex app-server diagnostic: {}", redact_text(&line));
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        })
        .map_err(|error| CodexRuntimeError::Transport(format!("stderr thread: {error}")))
}

fn spawn_notification_dispatch(
    receiver: mpsc::Receiver<CodexNotification>,
    subscribers: Arc<Mutex<HashMap<u64, NotificationCallback>>>,
) -> Result<JoinHandle<()>, CodexRuntimeError> {
    thread::Builder::new()
        .name("bsaigc-codex-events".to_string())
        .spawn(move || {
            while let Ok(notification) = receiver.recv() {
                let callbacks = lock_unpoisoned(&subscribers)
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                for callback in callbacks {
                    let notification = notification.clone();
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        callback(notification)
                    }))
                    .is_err()
                    {
                        eprintln!("codex notification subscriber panicked");
                    }
                }
            }
        })
        .map_err(|error| CodexRuntimeError::Transport(format!("event thread: {error}")))
}

fn route_message(
    shared: &Arc<RuntimeShared>,
    notification_sender: Option<&mpsc::Sender<CodexNotification>>,
    message: Value,
) {
    shared
        .last_message_at
        .store(now_millis(), Ordering::Release);
    if let Some(id) = message.get("id").and_then(Value::as_u64) {
        if message.get("method").is_none() {
            if let Some(sender) = lock_unpoisoned(&shared.pending).remove(&id) {
                let _ = sender.send(parse_response(message));
            }
            return;
        }
    }

    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return;
    };
    let raw_params = message.get("params").cloned().unwrap_or(Value::Null);
    let params = if method == DYNAMIC_TOOL_CALL_METHOD {
        // Tool arguments are consumed inside the trusted Rust host. Redacting
        // them here would corrupt valid tool inputs before authorization and
        // dispatch; UI-facing layers remain responsible for asset-id projection.
        raw_params
    } else {
        sanitize_for_host(raw_params)
    };
    let notification = CodexNotification {
        request_id: message.get("id").cloned(),
        method: method.to_string(),
        params,
        received_at: now_millis(),
    };
    if let Some(sender) = notification_sender {
        let _ = sender.send(notification);
    } else {
        dispatch_inline(&shared.subscribers, notification);
    }
}

fn parse_response(message: Value) -> Result<Value, CodexRuntimeError> {
    if let Some(error) = message.get("error") {
        return Err(CodexRuntimeError::Remote {
            code: error.get("code").and_then(Value::as_i64),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown app-server error")
                .to_string(),
            data: error.get("data").cloned(),
        });
    }
    message.get("result").cloned().ok_or_else(|| {
        CodexRuntimeError::Protocol("response contained neither result nor error".to_string())
    })
}

fn dispatch_inline(
    subscribers: &Arc<Mutex<HashMap<u64, NotificationCallback>>>,
    notification: CodexNotification,
) {
    let callbacks = lock_unpoisoned(subscribers)
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for callback in callbacks {
        callback(notification.clone());
    }
}

fn mark_dead(shared: &Arc<RuntimeShared>, failure: CodexRuntimeError) {
    shared.running.store(false, Ordering::Release);
    shared.initialized.store(false, Ordering::Release);
    let reason = failure.to_string();
    let mut exit_reason = lock_unpoisoned(&shared.exit_reason);
    if exit_reason.is_none() {
        *exit_reason = Some(reason);
    }
    drop(exit_reason);
    let pending = {
        let mut pending = lock_unpoisoned(&shared.pending);
        pending
            .drain()
            .map(|(_, sender)| sender)
            .collect::<Vec<_>>()
    };
    for sender in pending {
        let _ = sender.send(Err(failure.clone()));
    }
}

fn shutdown_inner(inner: &RuntimeInner) {
    if inner.shared.shutting_down.swap(true, Ordering::AcqRel) {
        return;
    }
    inner.shared.running.store(false, Ordering::Release);
    inner.shared.initialized.store(false, Ordering::Release);
    lock_unpoisoned(&inner.outbound).take();
    mark_dead(&inner.shared, CodexRuntimeError::ShuttingDown);
    terminate_shared_child(&inner.shared);
    lock_unpoisoned(&inner.notification_sender).take();

    join_optional(&inner.writer_join);
    join_optional(&inner.reader_join);
    join_optional(&inner.stderr_join);
    join_optional(&inner.notification_join);
}

fn refresh_process_status(shared: &Arc<RuntimeShared>) {
    if !shared.running.load(Ordering::Acquire) {
        return;
    }
    let status = {
        let mut child = lock_unpoisoned(&shared.child);
        child
            .as_mut()
            .and_then(|child| child.try_wait().ok())
            .flatten()
    };
    if let Some(status) = status {
        lock_unpoisoned(&shared.child).take();
        mark_dead(
            shared,
            CodexRuntimeError::ProcessExited(format!("process status {status}")),
        );
    }
}

fn terminate_shared_child(shared: &Arc<RuntimeShared>) {
    let child = lock_unpoisoned(&shared.child).take();
    if let Some(mut child) = child {
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
}

fn join_optional(slot: &Mutex<Option<JoinHandle<()>>>) {
    if let Some(handle) = lock_unpoisoned(slot).take() {
        join_worker(handle);
    }
}

fn join_worker(handle: JoinHandle<()>) {
    if handle.thread().id() != thread::current().id() {
        let _ = handle.join();
    }
}

fn take_stdin(child: &mut Child) -> Result<ChildStdin, CodexRuntimeError> {
    child.stdin.take().ok_or_else(|| {
        terminate_child(child);
        CodexRuntimeError::StdioUnavailable("stdin")
    })
}

fn take_stdout(child: &mut Child) -> Result<ChildStdout, CodexRuntimeError> {
    child.stdout.take().ok_or_else(|| {
        terminate_child(child);
        CodexRuntimeError::StdioUnavailable("stdout")
    })
}

fn take_stderr(child: &mut Child) -> Result<ChildStderr, CodexRuntimeError> {
    child.stderr.take().ok_or_else(|| {
        terminate_child(child);
        CodexRuntimeError::StdioUnavailable("stderr")
    })
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn current_exit_reason(shared: &Arc<RuntimeShared>) -> String {
    lock_unpoisoned(&shared.exit_reason)
        .clone()
        .unwrap_or_else(|| "runtime is not running".to_string())
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn nonzero(value: u64) -> Option<u64> {
    (value != 0).then_some(value)
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn sanitize_for_host(value: Value) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    let value = if is_path_key(&key) {
                        redact_path_value(value)
                    } else {
                        sanitize_for_host(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(sanitize_for_host).collect()),
        Value::String(value) if looks_like_absolute_path(&value) => {
            Value::String(REDACTED_PATH.to_string())
        }
        value => value,
    }
}

fn is_path_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "codexhome" | "cwd" | "path" | "instructionsources"
    )
}

fn redact_path_value(value: Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|_| Value::String(REDACTED_PATH.to_string()))
                .collect(),
        ),
        _ => Value::String(REDACTED_PATH.to_string()),
    }
}

fn looks_like_absolute_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.starts_with('/')
        || value.starts_with("\\\\")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/'))
}

fn redact_text(line: &str) -> String {
    line.split_whitespace()
        .map(|part| {
            if looks_like_absolute_path(part.trim_matches(['\'', '"', ',', ';'])) {
                REDACTED_PATH
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CodexRequestId {
    String(String),
    Integer(i64),
}

impl fmt::Display for CodexRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(value) => formatter.write_str(value),
            Self::Integer(value) => write!(formatter, "{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DynamicToolSpec {
    Function(DynamicToolFunctionSpec),
    Namespace(DynamicToolNamespaceSpec),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolFunctionSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(default, skip_serializing_if = "is_false")]
    pub defer_loading: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolNamespaceSpec {
    pub name: String,
    pub description: String,
    pub tools: Vec<DynamicToolNamespaceTool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DynamicToolNamespaceTool {
    Function(DynamicToolFunctionSpec),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallParams {
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicToolCallRequest {
    pub request_id: CodexRequestId,
    pub params: DynamicToolCallParams,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallResponse {
    pub content_items: Vec<DynamicToolCallOutputContentItem>,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DynamicToolCallOutputContentItem {
    InputText { text: String },
    InputImage { image_url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<Personality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_start_source: Option<ThreadStartSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_tools: Option<Vec<DynamicToolSpec>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeParams {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<Personality>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_key: Option<ThreadSortKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_direction: Option<SortDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_providers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kinds: Option<Vec<ThreadSourceKind>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<CwdFilter>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub use_state_db_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_term: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartParams {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_user_message_id: Option<String>,
    pub input: Vec<UserInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<SandboxPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ReasoningSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<Personality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterruptParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CwdFilter {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AskForApproval {
    Policy(ApprovalPolicy),
    Granular { granular: GranularApproval },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalPolicy {
    #[serde(rename = "untrusted")]
    Untrusted,
    #[serde(rename = "on-request")]
    OnRequest,
    #[serde(rename = "never")]
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GranularApproval {
    pub sandbox_approval: bool,
    pub rules: bool,
    pub skill_approval: bool,
    pub request_permissions: bool,
    pub mcp_elicitations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalsReviewer {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "auto_review")]
    AutoReview,
    #[serde(rename = "guardian_subagent")]
    GuardianSubagent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "workspace-write")]
    WorkspaceWrite,
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SandboxPolicy {
    #[serde(rename = "dangerFullAccess")]
    DangerFullAccess,
    #[serde(rename = "readOnly")]
    ReadOnly {
        #[serde(rename = "networkAccess")]
        network_access: bool,
    },
    #[serde(rename = "externalSandbox")]
    ExternalSandbox {
        #[serde(rename = "networkAccess")]
        network_access: NetworkAccess,
    },
    #[serde(rename = "workspaceWrite")]
    WorkspaceWrite {
        #[serde(rename = "writableRoots")]
        writable_roots: Vec<String>,
        #[serde(rename = "networkAccess")]
        network_access: bool,
        #[serde(rename = "excludeTmpdirEnvVar")]
        exclude_tmpdir_env_var: bool,
        #[serde(rename = "excludeSlashTmp")]
        exclude_slash_tmp: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkAccess {
    #[serde(rename = "restricted")]
    Restricted,
    #[serde(rename = "enabled")]
    Enabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Personality {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "friendly")]
    Friendly,
    #[serde(rename = "pragmatic")]
    Pragmatic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreadStartSource {
    #[serde(rename = "startup")]
    Startup,
    #[serde(rename = "clear")]
    Clear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreadSourceKind {
    #[serde(rename = "cli")]
    Cli,
    #[serde(rename = "vscode")]
    Vscode,
    #[serde(rename = "exec")]
    Exec,
    #[serde(rename = "appServer")]
    AppServer,
    #[serde(rename = "subAgent")]
    SubAgent,
    #[serde(rename = "subAgentReview")]
    SubAgentReview,
    #[serde(rename = "subAgentCompact")]
    SubAgentCompact,
    #[serde(rename = "subAgentThreadSpawn")]
    SubAgentThreadSpawn,
    #[serde(rename = "subAgentOther")]
    SubAgentOther,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreadSortKey {
    #[serde(rename = "created_at")]
    CreatedAt,
    #[serde(rename = "updated_at")]
    UpdatedAt,
    #[serde(rename = "recency_at")]
    RecencyAt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortDirection {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReasoningSummary {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "concise")]
    Concise,
    #[serde(rename = "detailed")]
    Detailed,
    #[serde(rename = "none")]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UserInput {
    #[serde(rename = "text")]
    Text {
        text: String,
        text_elements: Vec<TextElement>,
    },
    #[serde(rename = "image")]
    Image {
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
        url: String,
    },
    #[serde(rename = "localImage")]
    LocalImage {
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
        path: String,
    },
    #[serde(rename = "skill")]
    Skill { name: String, path: String },
    #[serde(rename = "mention")]
    Mention { name: String, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextElement {
    pub byte_range: ByteRange,
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageDetail {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "original")]
    Original,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_token_clones_share_the_same_signal() {
        let original = CancellationToken::new();
        let cloned = original.clone();

        assert!(!original.is_cancelled());
        assert!(!cloned.is_cancelled());

        cloned.cancel();

        assert!(original.is_cancelled());
        assert!(cloned.is_cancelled());
        assert_eq!(
            original.check_cancelled().unwrap_err().code,
            "CONTRACT_REVIEW_CANCELLED"
        );
    }

    fn test_runtime() -> CodexRuntime {
        let subscribers = Arc::new(Mutex::new(HashMap::new()));
        let shared = Arc::new(RuntimeShared {
            pending: Mutex::new(HashMap::new()),
            subscribers,
            child: Mutex::new(None),
            running: AtomicBool::new(true),
            initialized: AtomicBool::new(true),
            shutting_down: AtomicBool::new(false),
            exit_reason: Mutex::new(None),
            last_message_at: AtomicU64::new(0),
        });
        let (sender, receiver) = mpsc::channel::<String>();
        let writer = thread::spawn(move || while receiver.recv().is_ok() {});
        CodexRuntime {
            inner: Arc::new(RuntimeInner {
                shared,
                outbound: Mutex::new(Some(sender)),
                next_request_id: AtomicU64::new(1),
                next_subscription_id: AtomicU64::new(1),
                source: "test".to_string(),
                user_agent: Some("test".to_string()),
                platform_family: None,
                platform_os: None,
                started_at: now_millis(),
                reader_join: Mutex::new(None),
                writer_join: Mutex::new(Some(writer)),
                stderr_join: Mutex::new(None),
                notification_join: Mutex::new(None),
                notification_sender: Mutex::new(None),
            }),
        }
    }

    fn test_runtime_with_captured_outbound() -> (CodexRuntime, mpsc::Receiver<String>) {
        let subscribers = Arc::new(Mutex::new(HashMap::new()));
        let shared = Arc::new(RuntimeShared {
            pending: Mutex::new(HashMap::new()),
            subscribers,
            child: Mutex::new(None),
            running: AtomicBool::new(true),
            initialized: AtomicBool::new(true),
            shutting_down: AtomicBool::new(false),
            exit_reason: Mutex::new(None),
            last_message_at: AtomicU64::new(0),
        });
        let (sender, receiver) = mpsc::channel::<String>();
        let runtime = CodexRuntime {
            inner: Arc::new(RuntimeInner {
                shared,
                outbound: Mutex::new(Some(sender)),
                next_request_id: AtomicU64::new(1),
                next_subscription_id: AtomicU64::new(1),
                source: "test".to_string(),
                user_agent: Some("test".to_string()),
                platform_family: None,
                platform_os: None,
                started_at: now_millis(),
                reader_join: Mutex::new(None),
                writer_join: Mutex::new(None),
                stderr_join: Mutex::new(None),
                notification_join: Mutex::new(None),
                notification_sender: Mutex::new(None),
            }),
        };
        (runtime, receiver)
    }

    #[test]
    fn initialize_enables_experimental_api() {
        let params = initialize_params();
        assert_eq!(params["capabilities"]["experimentalApi"], true);
        assert_eq!(params["capabilities"]["requestAttestation"], false);
    }

    #[test]
    fn routes_response_to_matching_request() {
        let runtime = test_runtime();
        let (sender, receiver) = mpsc::channel();
        lock_unpoisoned(&runtime.inner.shared.pending).insert(42, sender);
        route_message(
            &runtime.inner.shared,
            None,
            json!({ "id": 42, "result": { "ok": true } }),
        );
        assert_eq!(receiver.recv().unwrap().unwrap(), json!({ "ok": true }));
        assert!(lock_unpoisoned(&runtime.inner.shared.pending).is_empty());
    }

    #[test]
    fn routes_sanitized_notifications_to_subscribers() {
        let runtime = test_runtime();
        let (sender, receiver) = mpsc::channel();
        let _subscription = runtime.subscribe(move |notification| {
            sender.send(notification).unwrap();
        });
        route_message(
            &runtime.inner.shared,
            None,
            json!({
                "method": "item/agentMessage/delta",
                "params": { "threadId": "t1", "delta": "hello", "cwd": "C:\\secret" }
            }),
        );
        let notification = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(notification.method, "item/agentMessage/delta");
        assert_eq!(notification.params["delta"], "hello");
        assert_eq!(notification.params["cwd"], REDACTED_PATH);
    }

    #[test]
    fn routes_dynamic_tool_requests_with_integer_and_string_ids() {
        let runtime = test_runtime();
        let (sender, receiver) = mpsc::channel();
        let _subscription = runtime.subscribe(move |notification| {
            sender.send(notification).unwrap();
        });

        for request_id in [json!(60), json!("tool-request-60")] {
            route_message(
                &runtime.inner.shared,
                None,
                json!({
                    "id": request_id,
                    "method": DYNAMIC_TOOL_CALL_METHOD,
                    "params": {
                        "threadId": "thr_123",
                        "turnId": "turn_123",
                        "callId": "call_123",
                        "namespace": "business",
                        "tool": "project_read",
                        "arguments": {
                            "projectId": "project-123",
                            "sourcePath": "C:\\contracts\\master.docx"
                        }
                    }
                }),
            );

            let notification = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
            let call = notification
                .dynamic_tool_call()
                .expect("dynamic tool request should parse")
                .expect("method should be recognized");
            assert_eq!(call.params.thread_id, "thr_123");
            assert_eq!(call.params.turn_id, "turn_123");
            assert_eq!(call.params.call_id, "call_123");
            assert_eq!(call.params.namespace.as_deref(), Some("business"));
            assert_eq!(call.params.tool, "project_read");
            assert_eq!(
                call.params.arguments["sourcePath"],
                "C:\\contracts\\master.docx"
            );
        }
    }

    #[test]
    fn dynamic_tool_response_preserves_request_id_and_omits_jsonrpc() {
        let (runtime, outbound) = test_runtime_with_captured_outbound();
        let response = || DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: r#"{"ok":true}"#.to_string(),
            }],
            success: true,
        };

        runtime
            .respond_dynamic_tool(CodexRequestId::Integer(60), response())
            .unwrap();
        runtime
            .respond_dynamic_tool(
                CodexRequestId::String("tool-request-60".to_string()),
                response(),
            )
            .unwrap();

        let numeric: Value =
            serde_json::from_str(&outbound.recv_timeout(Duration::from_secs(1)).unwrap()).unwrap();
        assert_eq!(
            numeric,
            json!({
                "id": 60,
                "result": {
                    "contentItems": [{ "type": "inputText", "text": "{\"ok\":true}" }],
                    "success": true
                }
            })
        );
        assert!(numeric.get("jsonrpc").is_none());

        let string: Value =
            serde_json::from_str(&outbound.recv_timeout(Duration::from_secs(1)).unwrap()).unwrap();
        assert_eq!(string["id"], "tool-request-60");
        assert!(string.get("jsonrpc").is_none());
    }

    #[test]
    fn request_deadline_removes_pending_entry() {
        let runtime = test_runtime();
        let error = runtime
            .request(
                "thread/list",
                json!({}),
                Instant::now() + Duration::from_millis(20),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            CodexRuntimeError::DeadlineExceeded { ref method } if method == "thread/list"
        ));
        assert!(lock_unpoisoned(&runtime.inner.shared.pending).is_empty());
    }

    #[test]
    fn process_exit_fails_every_pending_request() {
        let runtime = test_runtime();
        let (first_sender, first_receiver) = mpsc::channel();
        let (second_sender, second_receiver) = mpsc::channel();
        {
            let mut pending = lock_unpoisoned(&runtime.inner.shared.pending);
            pending.insert(1, first_sender);
            pending.insert(2, second_sender);
        }
        mark_dead(
            &runtime.inner.shared,
            CodexRuntimeError::ProcessExited("test exit".to_string()),
        );
        assert!(matches!(
            first_receiver.recv().unwrap(),
            Err(CodexRuntimeError::ProcessExited(ref reason)) if reason == "test exit"
        ));
        assert!(matches!(
            second_receiver.recv().unwrap(),
            Err(CodexRuntimeError::ProcessExited(ref reason)) if reason == "test exit"
        ));
    }

    #[test]
    fn wrappers_serialize_vendor_0144_field_names() {
        let dynamic_tools = vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
            name: "business".to_string(),
            description: "Business workspace tools".to_string(),
            tools: vec![DynamicToolNamespaceTool::Function(
                DynamicToolFunctionSpec {
                    name: "project_read".to_string(),
                    description: "Read project master data".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": { "projectId": { "type": "string" } },
                        "required": ["projectId"]
                    }),
                    defer_loading: false,
                },
            )],
        })];
        let dynamic_tools_value = serde_json::to_value(ThreadStartParams {
            dynamic_tools: Some(dynamic_tools),
            ..ThreadStartParams::default()
        })
        .unwrap();
        assert_eq!(
            dynamic_tools_value,
            json!({
                "dynamicTools": [{
                    "type": "namespace",
                    "name": "business",
                    "description": "Business workspace tools",
                    "tools": [{
                        "type": "function",
                        "name": "project_read",
                        "description": "Read project master data",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "projectId": { "type": "string" } },
                            "required": ["projectId"]
                        }
                    }]
                }]
            })
        );

        let thread_start = serde_json::to_value(ThreadStartParams {
            model_provider: Some("openai".to_string()),
            service_tier: Some(None),
            approval_policy: Some(AskForApproval::Policy(ApprovalPolicy::OnRequest)),
            session_start_source: Some(ThreadStartSource::Startup),
            ..ThreadStartParams::default()
        })
        .unwrap();
        assert_eq!(
            thread_start,
            json!({
                "modelProvider": "openai",
                "serviceTier": null,
                "approvalPolicy": "on-request",
                "sessionStartSource": "startup"
            })
        );

        let thread_list = serde_json::to_value(ThreadListParams {
            sort_key: Some(ThreadSortKey::UpdatedAt),
            source_kinds: Some(vec![ThreadSourceKind::AppServer]),
            use_state_db_only: true,
            ..ThreadListParams::default()
        })
        .unwrap();
        assert_eq!(
            thread_list,
            json!({
                "sortKey": "updated_at",
                "sourceKinds": ["appServer"],
                "useStateDbOnly": true
            })
        );

        let value = serde_json::to_value(TurnInterruptParams {
            thread_id: "thread".to_string(),
            turn_id: "turn".to_string(),
        })
        .unwrap();
        assert_eq!(value, json!({ "threadId": "thread", "turnId": "turn" }));

        let value = serde_json::to_value(UserInput::Text {
            text: "hello".to_string(),
            text_elements: Vec::new(),
        })
        .unwrap();
        assert_eq!(
            value,
            json!({ "type": "text", "text": "hello", "text_elements": [] })
        );

        let sandbox = serde_json::to_value(SandboxPolicy::WorkspaceWrite {
            writable_roots: vec!["workspace".to_string()],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        })
        .unwrap();
        assert_eq!(
            sandbox,
            json!({
                "type": "workspaceWrite",
                "writableRoots": ["workspace"],
                "networkAccess": false,
                "excludeTmpdirEnvVar": true,
                "excludeSlashTmp": false
            })
        );
    }

    #[test]
    #[ignore = "requires an installed official Codex CLI"]
    fn official_runtime_can_list_threads_without_starting_a_turn() {
        let root = tempfile::tempdir().expect("temporary runtime root");
        let workspace = root.path().join("brain-workspace");
        let runtime = CodexRuntime::start(&workspace).expect("official app-server should start");
        let response = runtime
            .thread_list(
                ThreadListParams {
                    limit: Some(1),
                    ..ThreadListParams::default()
                },
                Instant::now() + Duration::from_secs(10),
            )
            .expect("thread/list should succeed");
        assert!(response.get("data").and_then(Value::as_array).is_some());
        runtime.shutdown();
    }

    #[test]
    #[ignore = "requires an installed official Codex CLI"]
    fn official_runtime_accepts_dynamic_tool_schema() {
        let root = tempfile::tempdir().expect("temporary runtime root");
        let workspace = root.path().join("brain-workspace");
        let runtime = CodexRuntime::start(&workspace).expect("official app-server should start");
        let response = runtime
            .thread_start(
                ThreadStartParams {
                    cwd: Some(workspace.to_string_lossy().into_owned()),
                    ephemeral: Some(true),
                    dynamic_tools: Some(vec![DynamicToolSpec::Namespace(
                        DynamicToolNamespaceSpec {
                            name: "business".to_string(),
                            description: "Business tools".to_string(),
                            tools: vec![DynamicToolNamespaceTool::Function(
                                DynamicToolFunctionSpec {
                                    name: "project_read".to_string(),
                                    description: "Read project master data".to_string(),
                                    input_schema: json!({
                                        "type": "object",
                                        "additionalProperties": false,
                                        "required": ["projectId"],
                                        "properties": {
                                            "projectId": { "type": "string" }
                                        }
                                    }),
                                    defer_loading: false,
                                },
                            )],
                        },
                    )]),
                    ..ThreadStartParams::default()
                },
                Instant::now() + Duration::from_secs(10),
            )
            .expect("thread/start should accept dynamicTools");
        assert!(response
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .is_some());
        runtime.shutdown();
    }
}
