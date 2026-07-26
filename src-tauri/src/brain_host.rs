use crate::ai_credential_service::AiCredentialService;
use crate::brain_store;
use crate::business_tool_registry::{
    BusinessToolCall, BusinessToolContext, BusinessToolRegistry, BUSINESS_TOOL_NAMESPACE,
};
use crate::codex_runtime::{
    ApprovalPolicy, ApprovalsReviewer, AskForApproval, CodexNotification, CodexRequestId,
    CodexRuntime, CodexRuntimeError, CodexRuntimeHealth, CodexSubscription,
    DynamicToolCallOutputContentItem, DynamicToolCallRequest, DynamicToolCallResponse,
    DynamicToolFunctionSpec, DynamicToolNamespaceSpec, DynamicToolNamespaceTool, DynamicToolSpec,
    SandboxMode, SandboxPolicy, SortDirection, TextElement, ThreadListParams, ThreadResumeParams,
    ThreadSortKey, ThreadStartParams, TurnInterruptParams, TurnStartParams, UserInput,
};
use crate::protocol::{
    BrainHostHealth, BrainStreamEvent, BrainThreadRecord, BrainThreadStatus, BrainTurnRecord,
    BrainTurnStartResult, BrainTurnStatus, HostError, InterruptBrainTurnRequest,
    ListRemoteBrainThreadsRequest, RemoteBrainThreadPage, ResumeBrainThreadRequest,
    StartBrainThreadRequest, StartBrainTurnRequest,
};
use crate::security::{self, OperationEffect};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const TURN_START_TIMEOUT: Duration = Duration::from_secs(45);
const APPROVAL_INTERRUPT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_TITLE_CHARS: usize = 240;
const MAX_MODEL_CHARS: usize = 128;
const MAX_SEARCH_CHARS: usize = 240;
const MAX_CURSOR_CHARS: usize = 4_096;
const MAX_INPUT_CHARS: usize = 100_000;
const MAX_ASSISTANT_CHARS: usize = 2_000_000;
const MAX_ERROR_CHARS: usize = 16_000;
const MAX_OUTPUT_SCHEMA_BYTES: usize = 64 * 1024;
const BRAIN_ACTOR_ID: &str = "codex-runtime";
const BUSINESS_SYSTEM_PROMPT: &str =
    include_str!("../resources/business-prompts/business-system.md");

type BrainCallback = Arc<dyn Fn(BrainStreamEvent) + Send + Sync + 'static>;

pub struct BrainSubscription {
    id: u64,
    subscribers: Weak<Mutex<HashMap<u64, BrainCallback>>>,
}

impl Drop for BrainSubscription {
    fn drop(&mut self) {
        if let Some(subscribers) = self.subscribers.upgrade() {
            lock_unpoisoned(&subscribers).remove(&self.id);
        }
    }
}

struct RuntimeSession {
    runtime: CodexRuntime,
    _subscription: CodexSubscription,
}

struct BrainHostInner {
    connection: Mutex<Connection>,
    workspace_root: PathBuf,
    runtime: Mutex<Option<RuntimeSession>>,
    reducer: Mutex<NotificationReducer>,
    subscribers: Arc<Mutex<HashMap<u64, BrainCallback>>>,
    next_subscription_id: AtomicU64,
    last_error_code: Mutex<Option<String>>,
    ai_credentials: Option<Arc<AiCredentialService>>,
    business_tools: Option<BusinessToolRegistry>,
    dynamic_tool_requests: Mutex<HashSet<CodexRequestId>>,
}

#[derive(Clone)]
pub struct BrainHost {
    inner: Arc<BrainHostInner>,
}

#[derive(Debug, Clone)]
pub(crate) struct StructuredBrainTurnRequest {
    pub project_id: Option<String>,
    pub title: Option<String>,
    pub input_text: String,
    pub output_schema: Value,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructuredBrainTurnResult {
    pub thread_id: String,
    pub turn_id: String,
    pub assistant_text: String,
}

impl BrainHost {
    /// Opens the local Brain ledger without starting Codex. Runtime discovery and
    /// process startup are delayed until a remote operation is requested.
    #[cfg(test)]
    pub fn open(database_path: &Path, workspace_root: &Path) -> Result<Self, HostError> {
        Self::open_with_credentials(database_path, workspace_root, None, None)
    }

    pub(crate) fn open_with_services(
        database_path: &Path,
        workspace_root: &Path,
        ai_credentials: Arc<AiCredentialService>,
        business_tools: BusinessToolRegistry,
    ) -> Result<Self, HostError> {
        Self::open_with_credentials(
            database_path,
            workspace_root,
            Some(ai_credentials),
            Some(business_tools),
        )
    }

    fn open_with_credentials(
        database_path: &Path,
        workspace_root: &Path,
        ai_credentials: Option<Arc<AiCredentialService>>,
        business_tools: Option<BusinessToolRegistry>,
    ) -> Result<Self, HostError> {
        if let Some(parent) = database_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                HostError::internal(format!("create brain ledger directory failed: {error}"))
            })?;
        }
        fs::create_dir_all(workspace_root).map_err(|error| {
            HostError::internal(format!("create brain workspace failed: {error}"))
        })?;
        let workspace_root = workspace_root.canonicalize().map_err(|error| {
            HostError::internal(format!("resolve brain workspace failed: {error}"))
        })?;
        if !workspace_root.is_absolute() {
            return Err(HostError::validation(
                "brain workspace must resolve to an absolute directory",
            ));
        }

        let mut connection = Connection::open(database_path)
            .map_err(|error| HostError::internal(format!("open brain SQLite failed: {error}")))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| {
                HostError::internal(format!("configure brain SQLite failed: {error}"))
            })?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|error| {
                HostError::internal(format!("configure brain SQLite failed: {error}"))
            })?;
        brain_store::migrate(&connection)?;
        security::migrate(&connection)?;
        recover_interrupted_turns(&mut connection)?;

        Ok(Self {
            inner: Arc::new(BrainHostInner {
                connection: Mutex::new(connection),
                workspace_root,
                runtime: Mutex::new(None),
                reducer: Mutex::new(NotificationReducer::default()),
                subscribers: Arc::new(Mutex::new(HashMap::new())),
                next_subscription_id: AtomicU64::new(1),
                last_error_code: Mutex::new(None),
                ai_credentials,
                business_tools,
                dynamic_tool_requests: Mutex::new(HashSet::new()),
            }),
        })
    }

    pub fn start_thread(
        &self,
        request: StartBrainThreadRequest,
    ) -> Result<BrainThreadRecord, HostError> {
        validate_optional_id("projectId", request.project_id.as_deref())?;
        validate_optional_text("title", request.title.as_deref(), MAX_TITLE_CHARS)?;
        validate_model(request.model.as_deref())?;

        let runtime = self.ensure_runtime()?;
        let params = ThreadStartParams {
            model: request.model.clone(),
            cwd: Some(self.workspace_string()),
            approval_policy: Some(AskForApproval::Policy(ApprovalPolicy::OnRequest)),
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            developer_instructions: Some(fixed_developer_instructions()),
            ephemeral: Some(false),
            service_name: Some("bsaigc-desktop".to_string()),
            dynamic_tools: self
                .inner
                .business_tools
                .as_ref()
                .map(|_| business_dynamic_tool_specs()),
            ..ThreadStartParams::default()
        };
        let response = runtime
            .thread_start(params, Instant::now() + REQUEST_TIMEOUT)
            .map_err(|error| map_non_replayable_error("thread/start", error))?;
        let record =
            thread_from_response(&response, request.project_id, request.title, request.model)?;
        self.upsert_thread(&record)
    }

    pub fn resume_thread(
        &self,
        request: ResumeBrainThreadRequest,
    ) -> Result<BrainThreadRecord, HostError> {
        validate_id("threadId", &request.thread_id)?;
        validate_optional_id("projectId", request.project_id.as_deref())?;
        validate_optional_text("title", request.title.as_deref(), MAX_TITLE_CHARS)?;
        validate_model(request.model.as_deref())?;

        let runtime = self.ensure_runtime()?;
        let params = ThreadResumeParams {
            thread_id: request.thread_id,
            model: request.model.clone(),
            model_provider: None,
            service_tier: None,
            cwd: Some(self.workspace_string()),
            approval_policy: Some(AskForApproval::Policy(ApprovalPolicy::OnRequest)),
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            config: None,
            base_instructions: None,
            developer_instructions: Some(fixed_developer_instructions()),
            personality: None,
        };
        let response = runtime
            .thread_resume(params, Instant::now() + REQUEST_TIMEOUT)
            .map_err(|error| map_non_replayable_error("thread/resume", error))?;
        let record =
            thread_from_response(&response, request.project_id, request.title, request.model)?;
        self.upsert_thread(&record)
    }

    pub fn list_remote_threads(
        &self,
        request: ListRemoteBrainThreadsRequest,
    ) -> Result<RemoteBrainThreadPage, HostError> {
        validate_optional_text("cursor", request.cursor.as_deref(), MAX_CURSOR_CHARS)?;
        validate_optional_text(
            "searchTerm",
            request.search_term.as_deref(),
            MAX_SEARCH_CHARS,
        )?;
        let limit = request.limit.unwrap_or(50);
        if !(1..=100).contains(&limit) {
            return Err(HostError::validation(
                "remote thread limit must be between 1 and 100",
            ));
        }

        let runtime = self.ensure_runtime()?;
        let response = runtime
            .thread_list(
                ThreadListParams {
                    cursor: request.cursor,
                    limit: Some(limit),
                    sort_key: Some(ThreadSortKey::UpdatedAt),
                    sort_direction: Some(SortDirection::Desc),
                    archived: request.archived,
                    search_term: request.search_term,
                    ..ThreadListParams::default()
                },
                Instant::now() + REQUEST_TIMEOUT,
            )
            .map_err(|error| map_runtime_error("thread/list", error, true))?;

        let values = response
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| protocol_error("thread/list returned no data array"))?;
        let mut threads = Vec::with_capacity(values.len());
        for value in values {
            let mut record = thread_from_value(value, None, None, None)?;
            if let Ok(existing) = self.get_local_thread(&record.id) {
                record.project_id = existing.project_id;
                if record.title.is_none() {
                    record.title = existing.title;
                }
                if record.model.is_none() {
                    record.model = existing.model;
                }
            }
            threads.push(self.upsert_thread(&record)?);
        }
        Ok(RemoteBrainThreadPage {
            threads,
            next_cursor: response
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    pub fn start_turn(
        &self,
        request: StartBrainTurnRequest,
    ) -> Result<BrainTurnStartResult, HostError> {
        validate_id("threadId", &request.thread_id)?;
        validate_input(&request.input_text)?;
        validate_model(request.model.as_deref())?;
        validate_effort(request.effort.as_deref())?;

        let runtime = self.ensure_runtime()?;
        let now = now_millis();
        let local_turn_id = format!("local-{}", Uuid::new_v4());
        let running = BrainTurnRecord {
            id: local_turn_id.clone(),
            thread_id: request.thread_id.clone(),
            status: BrainTurnStatus::Running,
            input_text: request.input_text.clone(),
            assistant_text: String::new(),
            error: None,
            created_at: now,
            updated_at: now,
        };
        {
            let connection = lock_connection(&self.inner.connection)?;
            brain_store::insert_turn(&connection, &running)?;
        }
        lock_unpoisoned(&self.inner.reducer).register_pending(&request.thread_id, &local_turn_id);
        self.update_thread_status(&request.thread_id, BrainThreadStatus::Running)?;

        let params = TurnStartParams {
            thread_id: request.thread_id.clone(),
            client_user_message_id: Some(local_turn_id.clone()),
            input: vec![UserInput::Text {
                text: request.input_text,
                text_elements: Vec::<TextElement>::new(),
            }],
            cwd: Some(self.workspace_string()),
            approval_policy: Some(AskForApproval::Policy(ApprovalPolicy::OnRequest)),
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox_policy: Some(self.fixed_sandbox_policy()),
            model: request.model,
            service_tier: None,
            effort: request.effort,
            summary: None,
            personality: None,
            output_schema: None,
        };
        let response = match runtime.turn_start(params, Instant::now() + TURN_START_TIMEOUT) {
            Ok(response) => response,
            Err(error) if outcome_is_unknown(&error) => {
                return Err(HostError::new(
                    "BRAIN_TURN_OUTCOME_UNKNOWN",
                    "Codex turn submission outcome is unknown; the host will not replay it",
                    false,
                ));
            }
            Err(error) => {
                lock_unpoisoned(&self.inner.reducer).discard_pending(&local_turn_id);
                self.finish_turn_if_running(
                    &local_turn_id,
                    BrainTurnStatus::Failed,
                    "",
                    Some("Codex rejected the turn before execution"),
                )?;
                self.update_thread_status(&request.thread_id, BrainThreadStatus::Ready)?;
                return Err(map_runtime_error("turn/start", error, false));
            }
        };
        let remote_turn_id = response
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or_else(|| protocol_error("turn/start returned no turn id"))?
            .to_string();
        validate_id("remoteTurnId", &remote_turn_id)?;
        lock_unpoisoned(&self.inner.reducer).bind_remote_turn(&remote_turn_id, &local_turn_id);

        Ok(BrainTurnStartResult {
            turn: self.get_local_turn(&local_turn_id)?,
            remote_turn_id,
        })
    }

    /// Runs one read-only Codex turn and waits for its durable local turn record
    /// to become terminal. This is used by backend business stages that require
    /// strict structured output and must not move Agent execution into React.
    pub(crate) fn run_structured_turn(
        &self,
        request: StructuredBrainTurnRequest,
    ) -> Result<StructuredBrainTurnResult, HostError> {
        validate_optional_id("projectId", request.project_id.as_deref())?;
        validate_optional_text("title", request.title.as_deref(), MAX_TITLE_CHARS)?;
        validate_input(&request.input_text)?;
        validate_model(request.model.as_deref())?;
        validate_effort(request.effort.as_deref())?;
        if request.timeout.is_zero() {
            return Err(HostError::validation(
                "structured Brain turn timeout must be positive",
            ));
        }
        if !request.output_schema.is_object() {
            return Err(HostError::validation(
                "structured Brain output schema must be a JSON object",
            ));
        }
        let schema_size = serde_json::to_vec(&request.output_schema)
            .map_err(|error| HostError::validation(format!("invalid output schema: {error}")))?
            .len();
        if schema_size > MAX_OUTPUT_SCHEMA_BYTES {
            return Err(HostError::validation(
                "structured Brain output schema exceeds 64 KiB",
            ));
        }

        let thread_record = self.start_thread(StartBrainThreadRequest {
            project_id: request.project_id,
            title: request.title,
            model: request.model.clone(),
        })?;
        let runtime = self.ensure_runtime()?;
        let now = now_millis();
        let local_turn_id = format!("local-{}", Uuid::new_v4());
        let running = BrainTurnRecord {
            id: local_turn_id.clone(),
            thread_id: thread_record.id.clone(),
            status: BrainTurnStatus::Running,
            input_text: request.input_text.clone(),
            assistant_text: String::new(),
            error: None,
            created_at: now,
            updated_at: now,
        };
        {
            let connection = lock_connection(&self.inner.connection)?;
            brain_store::insert_turn(&connection, &running)?;
        }
        lock_unpoisoned(&self.inner.reducer).register_pending(&thread_record.id, &local_turn_id);
        self.update_thread_status(&thread_record.id, BrainThreadStatus::Running)?;

        let params = TurnStartParams {
            thread_id: thread_record.id.clone(),
            client_user_message_id: Some(local_turn_id.clone()),
            input: vec![UserInput::Text {
                text: request.input_text,
                text_elements: Vec::<TextElement>::new(),
            }],
            cwd: Some(self.workspace_string()),
            approval_policy: Some(AskForApproval::Policy(ApprovalPolicy::Never)),
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox_policy: Some(SandboxPolicy::ReadOnly {
                network_access: false,
            }),
            model: request.model,
            service_tier: None,
            effort: request.effort,
            summary: None,
            personality: None,
            output_schema: Some(request.output_schema),
        };
        let response = match runtime.turn_start(params, Instant::now() + TURN_START_TIMEOUT) {
            Ok(response) => response,
            Err(error) if outcome_is_unknown(&error) => {
                return Err(HostError::new(
                    "BRAIN_TURN_OUTCOME_UNKNOWN",
                    "Codex structured turn submission outcome is unknown; the host will not replay it",
                    false,
                ));
            }
            Err(error) => {
                lock_unpoisoned(&self.inner.reducer).discard_pending(&local_turn_id);
                self.finish_turn_if_running(
                    &local_turn_id,
                    BrainTurnStatus::Failed,
                    "",
                    Some("Codex rejected the structured turn before execution"),
                )?;
                self.update_thread_status(&thread_record.id, BrainThreadStatus::Ready)?;
                return Err(map_runtime_error("turn/start", error, false));
            }
        };
        let remote_turn_id = response
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or_else(|| protocol_error("turn/start returned no turn id"))?
            .to_string();
        validate_id("remoteTurnId", &remote_turn_id)?;
        lock_unpoisoned(&self.inner.reducer).bind_remote_turn(&remote_turn_id, &local_turn_id);

        let deadline = Instant::now() + request.timeout;
        loop {
            let turn = self.get_local_turn(&local_turn_id)?;
            match turn.status {
                BrainTurnStatus::Completed => {
                    return Ok(StructuredBrainTurnResult {
                        thread_id: thread_record.id,
                        turn_id: local_turn_id,
                        assistant_text: turn.assistant_text,
                    });
                }
                BrainTurnStatus::Failed => {
                    return Err(HostError::new(
                        "CONTRACT_AGENT_TURN_FAILED",
                        turn.error
                            .unwrap_or_else(|| "Codex structured turn failed".to_string()),
                        true,
                    ));
                }
                BrainTurnStatus::Interrupted => {
                    return Err(HostError::new(
                        "CONTRACT_AGENT_TURN_INTERRUPTED",
                        "Codex structured turn was interrupted",
                        true,
                    ));
                }
                BrainTurnStatus::Running => {}
            }
            if Instant::now() >= deadline {
                let _ = runtime.turn_interrupt(
                    TurnInterruptParams {
                        thread_id: thread_record.id.clone(),
                        turn_id: remote_turn_id.clone(),
                    },
                    Instant::now() + APPROVAL_INTERRUPT_TIMEOUT,
                );
                lock_unpoisoned(&self.inner.reducer).mark_interrupted(&local_turn_id);
                self.finish_turn_if_running(
                    &local_turn_id,
                    BrainTurnStatus::Interrupted,
                    "",
                    Some("Codex structured turn exceeded its backend deadline"),
                )?;
                self.update_thread_status(&thread_record.id, BrainThreadStatus::Ready)?;
                return Err(HostError::new(
                    "CONTRACT_AGENT_TURN_TIMEOUT",
                    "Codex structured turn exceeded its backend deadline",
                    true,
                ));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn interrupt_turn(
        &self,
        request: InterruptBrainTurnRequest,
    ) -> Result<BrainTurnRecord, HostError> {
        validate_id("threadId", &request.thread_id)?;
        validate_id("turnId", &request.turn_id)?;
        let local = self.get_local_turn(&request.turn_id)?;
        if local.thread_id != request.thread_id {
            return Err(HostError::validation(
                "turnId does not belong to the requested thread",
            ));
        }
        if local.status != BrainTurnStatus::Running {
            return Ok(local);
        }
        let remote_turn_id = lock_unpoisoned(&self.inner.reducer)
            .remote_for_local(&request.turn_id)
            .ok_or_else(|| {
                HostError::new(
                    "BRAIN_REMOTE_TURN_PENDING",
                    "Codex has not acknowledged this turn yet",
                    true,
                )
            })?;
        let runtime = self.ensure_runtime()?;
        runtime
            .turn_interrupt(
                TurnInterruptParams {
                    thread_id: request.thread_id.clone(),
                    turn_id: remote_turn_id,
                },
                Instant::now() + REQUEST_TIMEOUT,
            )
            .map_err(|error| map_non_replayable_error("turn/interrupt", error))?;

        let assistant = lock_unpoisoned(&self.inner.reducer)
            .mark_interrupted(&request.turn_id)
            .unwrap_or_default();
        self.finish_turn_if_running(
            &request.turn_id,
            BrainTurnStatus::Interrupted,
            &assistant,
            None,
        )?;
        self.update_thread_status(&request.thread_id, BrainThreadStatus::Ready)?;
        self.get_local_turn(&request.turn_id)
    }

    pub fn list_local_threads(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<BrainThreadRecord>, HostError> {
        validate_optional_id("projectId", project_id)?;
        let connection = lock_connection(&self.inner.connection)?;
        brain_store::list_threads(&connection, project_id)
    }

    /// Archives or restores a local conversation. Running threads must be
    /// interrupted first so an in-flight turn never writes into a hidden
    /// thread unnoticed.
    pub fn archive_local_thread(
        &self,
        thread_id: &str,
        archived: bool,
    ) -> Result<BrainThreadRecord, HostError> {
        validate_id("threadId", thread_id)?;
        let connection = lock_connection(&self.inner.connection)?;
        let current = brain_store::get_thread(&connection, thread_id)?;
        if current.status == BrainThreadStatus::Running {
            return Err(HostError::new(
                "BRAIN_THREAD_RUNNING",
                "conversation is still running; interrupt it first",
                false,
            ));
        }
        let next = if archived {
            BrainThreadStatus::Archived
        } else {
            BrainThreadStatus::Ready
        };
        brain_store::set_thread_status(&connection, thread_id, &next, now_millis())
    }

    /// Permanently deletes a local conversation and its turns.
    pub fn delete_local_thread(&self, thread_id: &str) -> Result<(), HostError> {
        validate_id("threadId", thread_id)?;
        let connection = lock_connection(&self.inner.connection)?;
        let current = brain_store::get_thread(&connection, thread_id)?;
        if current.status == BrainThreadStatus::Running {
            return Err(HostError::new(
                "BRAIN_THREAD_RUNNING",
                "conversation is still running; interrupt it first",
                false,
            ));
        }
        brain_store::delete_thread(&connection, thread_id)
    }

    pub fn list_local_turns(&self, thread_id: &str) -> Result<Vec<BrainTurnRecord>, HostError> {
        validate_id("threadId", thread_id)?;
        let connection = lock_connection(&self.inner.connection)?;
        brain_store::list_turns(&connection, thread_id)
    }

    pub fn subscribe<F>(&self, callback: F) -> BrainSubscription
    where
        F: Fn(BrainStreamEvent) + Send + Sync + 'static,
    {
        let id = self
            .inner
            .next_subscription_id
            .fetch_add(1, Ordering::Relaxed);
        lock_unpoisoned(&self.inner.subscribers).insert(id, Arc::new(callback));
        BrainSubscription {
            id,
            subscribers: Arc::downgrade(&self.inner.subscribers),
        }
    }

    pub fn health(&self) -> BrainHostHealth {
        let runtime = lock_unpoisoned(&self.inner.runtime);
        let health = runtime.as_ref().map(|session| session.runtime.health());
        health_from_runtime(
            health.as_ref(),
            lock_unpoisoned(&self.inner.subscribers).len(),
            lock_unpoisoned(&self.inner.last_error_code).clone(),
        )
    }

    pub fn shutdown(&self) {
        if let Some(session) = lock_unpoisoned(&self.inner.runtime).take() {
            session.runtime.shutdown();
        }
        lock_unpoisoned(&self.inner.dynamic_tool_requests).clear();
    }

    pub(crate) fn refresh_credentials(&self) {
        self.shutdown();
        *lock_unpoisoned(&self.inner.last_error_code) = None;
    }

    fn ensure_runtime(&self) -> Result<CodexRuntime, HostError> {
        let mut slot = lock_unpoisoned(&self.inner.runtime);
        if let Some(session) = slot.as_ref() {
            if session.runtime.health().running {
                return Ok(session.runtime.clone());
            }
        }
        if let Some(session) = slot.take() {
            session.runtime.shutdown();
        }

        let provider = self
            .inner
            .ai_credentials
            .as_ref()
            .map(|service| service.load_runtime_provider())
            .transpose()?
            .flatten();
        let runtime = CodexRuntime::start_with_provider(
            &self.inner.workspace_root,
            provider.as_ref().map(|provider| provider.api_key.as_str()),
            provider.as_ref().map(|provider| provider.base_url.as_str()),
            provider.as_ref().map(|provider| provider.model.as_str()),
        )
        .map_err(|error| {
            eprintln!("brain Codex runtime startup diagnostic: {error}");
            let code = "BRAIN_RUNTIME_UNAVAILABLE".to_string();
            *lock_unpoisoned(&self.inner.last_error_code) = Some(code.clone());
            HostError::new(code, "Official Codex app-server is unavailable", true)
        })?;
        let weak = Arc::downgrade(&self.inner);
        let subscription = runtime.subscribe(move |notification| {
            if let Some(inner) = weak.upgrade() {
                handle_notification(&inner, notification);
            }
        });
        *lock_unpoisoned(&self.inner.last_error_code) = None;
        *slot = Some(RuntimeSession {
            runtime: runtime.clone(),
            _subscription: subscription,
        });
        Ok(runtime)
    }

    fn fixed_sandbox_policy(&self) -> SandboxPolicy {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![self.workspace_string()],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        }
    }

    fn workspace_string(&self) -> String {
        self.inner.workspace_root.to_string_lossy().into_owned()
    }

    fn upsert_thread(&self, record: &BrainThreadRecord) -> Result<BrainThreadRecord, HostError> {
        let connection = lock_connection(&self.inner.connection)?;
        brain_store::upsert_thread(&connection, record)
    }

    fn get_local_thread(&self, thread_id: &str) -> Result<BrainThreadRecord, HostError> {
        let connection = lock_connection(&self.inner.connection)?;
        brain_store::get_thread(&connection, thread_id)
    }

    fn get_local_turn(&self, turn_id: &str) -> Result<BrainTurnRecord, HostError> {
        let connection = lock_connection(&self.inner.connection)?;
        brain_store::get_turn(&connection, turn_id)
    }

    fn update_thread_status(
        &self,
        thread_id: &str,
        status: BrainThreadStatus,
    ) -> Result<(), HostError> {
        let connection = lock_connection(&self.inner.connection)?;
        let mut thread = brain_store::get_thread(&connection, thread_id)?;
        thread.status = status;
        thread.updated_at = now_millis();
        brain_store::upsert_thread(&connection, &thread)?;
        Ok(())
    }

    fn finish_turn_if_running(
        &self,
        turn_id: &str,
        status: BrainTurnStatus,
        assistant_text: &str,
        error: Option<&str>,
    ) -> Result<(), HostError> {
        let connection = lock_connection(&self.inner.connection)?;
        let current = brain_store::get_turn(&connection, turn_id)?;
        if current.status == BrainTurnStatus::Running {
            brain_store::finish_turn(
                &connection,
                turn_id,
                status,
                assistant_text,
                error,
                now_millis(),
            )?;
        }
        Ok(())
    }
}

fn recover_interrupted_turns(connection: &mut Connection) -> Result<(), HostError> {
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| HostError::internal(format!("recover brain turns failed: {error}")))?;
    let running = {
        let mut statement = transaction
            .prepare(
                "SELECT id, thread_id FROM brain_turns WHERE status = 'running' \
                 ORDER BY created_at, id",
            )
            .map_err(|error| HostError::internal(format!("recover brain turns failed: {error}")))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| HostError::internal(format!("recover brain turns failed: {error}")))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| HostError::internal(format!("recover brain turns failed: {error}")))?
    };
    let now = now_millis();
    for (turn_id, thread_id) in running {
        brain_store::finish_turn(
            &transaction,
            &turn_id,
            BrainTurnStatus::Interrupted,
            "",
            Some("Host restarted before Codex turn completion; turn was not replayed"),
            now,
        )?;
        let mut thread = brain_store::get_thread(&transaction, &thread_id)?;
        thread.status = BrainThreadStatus::Error;
        thread.updated_at = now.max(thread.updated_at);
        brain_store::upsert_thread(&transaction, &thread)?;
    }
    transaction
        .commit()
        .map_err(|error| HostError::internal(format!("recover brain turns failed: {error}")))
}

fn handle_notification(inner: &Arc<BrainHostInner>, notification: CodexNotification) {
    if notification.method == "item/tool/call" {
        handle_dynamic_tool_notification(inner, notification);
        return;
    }
    let reduced = lock_unpoisoned(&inner.reducer).reduce(notification);
    let Some(mut reduced) = reduced else {
        return;
    };

    let persisted = persist_reduced(inner, &mut reduced);
    if let Err(error) = persisted {
        eprintln!(
            "brain notification persistence failed: code={} retryable={}",
            error.code, error.retryable
        );
        return;
    }

    if let Some(approval) = reduced.approval {
        interrupt_for_approval(inner, approval);
    }
    dispatch_event(&inner.subscribers, reduced.event);
}

fn handle_dynamic_tool_notification(inner: &Arc<BrainHostInner>, notification: CodexNotification) {
    let parsed_id = notification.parsed_request_id().ok().flatten();
    let call = match notification.dynamic_tool_call() {
        Ok(Some(call)) => call,
        Ok(None) => return,
        Err(_) => {
            if let (Some(runtime), Some(request_id)) = (active_runtime(inner), parsed_id) {
                let _ = runtime.respond_dynamic_tool(
                    request_id,
                    failed_dynamic_tool_response(
                        "BUSINESS_TOOL_PROTOCOL_INVALID",
                        "dynamic tool request does not match the supported protocol",
                        false,
                    ),
                );
            }
            return;
        }
    };
    if !lock_unpoisoned(&inner.dynamic_tool_requests).insert(call.request_id.clone()) {
        return;
    }
    let Some(runtime) = active_runtime(inner) else {
        return;
    };
    let Some(registry) = inner.business_tools.clone() else {
        let _ = runtime.respond_dynamic_tool(
            call.request_id,
            failed_dynamic_tool_response(
                "BUSINESS_TOOL_REGISTRY_UNAVAILABLE",
                "business tool registry is unavailable",
                true,
            ),
        );
        return;
    };
    let project_id = project_id_for_thread(inner, &call.params.thread_id);
    let request_id = call.request_id.clone();
    let spawn = thread::Builder::new()
        .name("bsaigc-business-tool".to_string())
        .spawn(move || {
            let response = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dispatch_dynamic_tool(&registry, project_id, call)
            }))
            .unwrap_or_else(|_| {
                failed_dynamic_tool_response(
                    "BUSINESS_TOOL_HOST_PANIC",
                    "business tool execution failed unexpectedly",
                    true,
                )
            });
            if let Err(error) = runtime.respond_dynamic_tool(request_id, response) {
                eprintln!("business dynamic tool response failed: {error}");
            }
        });
    if let Err(error) = spawn {
        eprintln!("business dynamic tool worker could not start: {error}");
    }
}

fn active_runtime(inner: &BrainHostInner) -> Option<CodexRuntime> {
    lock_unpoisoned(&inner.runtime)
        .as_ref()
        .map(|session| session.runtime.clone())
}

fn project_id_for_thread(inner: &BrainHostInner, thread_id: &str) -> Option<String> {
    let connection = lock_connection(&inner.connection).ok()?;
    brain_store::get_thread(&connection, thread_id)
        .ok()
        .and_then(|thread| thread.project_id)
}

fn dispatch_dynamic_tool(
    registry: &BusinessToolRegistry,
    project_id: Option<String>,
    call: DynamicToolCallRequest,
) -> DynamicToolCallResponse {
    let context = BusinessToolContext {
        call_id: call.params.call_id,
        actor_id: BRAIN_ACTOR_ID.to_string(),
        account_id: None,
        project_id,
        trace_id: format!("brain-tool:{}", call.params.turn_id),
    };
    let tool_call = BusinessToolCall {
        namespace: call.params.namespace.unwrap_or_default(),
        tool: call.params.tool,
        arguments: call.params.arguments,
    };
    match registry.dispatch(&context, tool_call) {
        Ok(result) => DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: json!({ "ok": true, "result": result }).to_string(),
            }],
            success: true,
        },
        Err(error) => DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: json!({ "ok": false, "error": error }).to_string(),
            }],
            success: false,
        },
    }
}

fn failed_dynamic_tool_response(
    code: &str,
    message: &str,
    retryable: bool,
) -> DynamicToolCallResponse {
    DynamicToolCallResponse {
        content_items: vec![DynamicToolCallOutputContentItem::InputText {
            text: json!({
                "ok": false,
                "error": {
                    "code": code,
                    "message": message,
                    "retryable": retryable
                }
            })
            .to_string(),
        }],
        success: false,
    }
}

fn persist_reduced(
    inner: &BrainHostInner,
    reduced: &mut ReducedNotification,
) -> Result<(), HostError> {
    let connection = lock_connection(&inner.connection)?;
    if let Some(thread_change) = reduced.thread_change.as_ref() {
        persist_thread_change(&connection, thread_change)?;
    }
    if let Some(completion) = reduced.completion.as_ref() {
        let current = brain_store::get_turn(&connection, &completion.local_turn_id)?;
        if current.status == BrainTurnStatus::Running {
            brain_store::finish_turn(
                &connection,
                &completion.local_turn_id,
                completion.status.clone(),
                &completion.assistant_text,
                completion.error.as_deref(),
                completion.occurred_at,
            )?;
        }
    }
    if let Some(approval) = reduced.approval.as_ref() {
        let decision = security::authorize(
            &connection,
            BRAIN_ACTOR_ID,
            approval.operation,
            "brainServerRequest",
            Some(&approval.resource_id),
            OperationEffect::Irreversible,
            None,
        )?;
        reduced.event.payload = Some(json!({
            "requestType": approval.operation,
            "approvalId": decision.approval_id,
            "allowed": false
        }));
    }
    Ok(())
}

fn persist_thread_change(connection: &Connection, change: &ThreadChange) -> Result<(), HostError> {
    let existing = brain_store::get_thread(connection, &change.thread_id);
    let mut record = match existing {
        Ok(record) => record,
        Err(error) if error.code == "BRAIN_THREAD_NOT_FOUND" => BrainThreadRecord {
            id: change.thread_id.clone(),
            project_id: None,
            title: change.title.clone(),
            model: change.model.clone(),
            status: change.status.clone(),
            created_at: change.occurred_at,
            updated_at: change.occurred_at,
        },
        Err(error) => return Err(error),
    };
    record.status = change.status.clone();
    if record.title.is_none() {
        record.title = change.title.clone();
    }
    if record.model.is_none() {
        record.model = change.model.clone();
    }
    record.updated_at = change.occurred_at.max(record.updated_at);
    brain_store::upsert_thread(connection, &record)?;
    Ok(())
}

fn interrupt_for_approval(inner: &Arc<BrainHostInner>, approval: ApprovalIntent) {
    let (Some(thread_id), Some(turn_id)) = (approval.thread_id, approval.remote_turn_id) else {
        return;
    };
    let runtime = lock_unpoisoned(&inner.runtime)
        .as_ref()
        .map(|session| session.runtime.clone());
    let Some(runtime) = runtime else {
        return;
    };

    // CodexRuntime intentionally does not expose raw JSON-RPC response writes.
    // Interrupting the turn is the conservative first-version rejection path.
    let _ = thread::Builder::new()
        .name("bsaigc-brain-approval-deny".to_string())
        .spawn(move || {
            let _ = runtime.turn_interrupt(
                TurnInterruptParams { thread_id, turn_id },
                Instant::now() + APPROVAL_INTERRUPT_TIMEOUT,
            );
        });
}

fn dispatch_event(subscribers: &Arc<Mutex<HashMap<u64, BrainCallback>>>, event: BrainStreamEvent) {
    let callbacks = lock_unpoisoned(subscribers)
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for callback in callbacks {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(event.clone())))
            .is_err()
        {
            eprintln!("brain event subscriber panicked");
        }
    }
}

#[derive(Debug, Clone)]
struct ThreadChange {
    thread_id: String,
    title: Option<String>,
    model: Option<String>,
    status: BrainThreadStatus,
    occurred_at: i64,
}

#[derive(Debug, Clone)]
struct TurnCompletion {
    local_turn_id: String,
    status: BrainTurnStatus,
    assistant_text: String,
    error: Option<String>,
    occurred_at: i64,
}

#[derive(Debug, Clone)]
struct ApprovalIntent {
    operation: &'static str,
    resource_id: String,
    thread_id: Option<String>,
    remote_turn_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ReducedNotification {
    event: BrainStreamEvent,
    thread_change: Option<ThreadChange>,
    completion: Option<TurnCompletion>,
    approval: Option<ApprovalIntent>,
}

#[derive(Debug, Default)]
struct TurnAccumulator {
    local_turn_id: String,
    assistant_text: String,
    error: Option<String>,
}

#[derive(Debug, Default)]
struct NotificationReducer {
    sequence: i64,
    pending_by_thread: HashMap<String, VecDeque<String>>,
    remote_by_local: HashMap<String, String>,
    turns_by_remote: HashMap<String, TurnAccumulator>,
    terminal_local_turns: HashSet<String>,
}

impl NotificationReducer {
    fn register_pending(&mut self, thread_id: &str, local_turn_id: &str) {
        self.pending_by_thread
            .entry(thread_id.to_string())
            .or_default()
            .push_back(local_turn_id.to_string());
    }

    fn discard_pending(&mut self, local_turn_id: &str) {
        for pending in self.pending_by_thread.values_mut() {
            pending.retain(|candidate| candidate != local_turn_id);
        }
        self.pending_by_thread
            .retain(|_, pending| !pending.is_empty());
    }

    fn bind_remote_turn(&mut self, remote_turn_id: &str, local_turn_id: &str) {
        self.discard_pending(local_turn_id);
        self.remote_by_local
            .insert(local_turn_id.to_string(), remote_turn_id.to_string());
        self.turns_by_remote
            .entry(remote_turn_id.to_string())
            .or_insert_with(|| TurnAccumulator {
                local_turn_id: local_turn_id.to_string(),
                assistant_text: String::new(),
                error: None,
            });
    }

    fn remote_for_local(&self, local_turn_id: &str) -> Option<String> {
        self.remote_by_local.get(local_turn_id).cloned()
    }

    fn mark_interrupted(&mut self, local_turn_id: &str) -> Option<String> {
        self.terminal_local_turns.insert(local_turn_id.to_string());
        let remote = self.remote_by_local.get(local_turn_id)?;
        self.turns_by_remote
            .get(remote)
            .map(|turn| turn.assistant_text.clone())
    }

    fn reduce(&mut self, notification: CodexNotification) -> Option<ReducedNotification> {
        let occurred_at = i64::try_from(notification.received_at).unwrap_or(i64::MAX);
        match notification.method.as_str() {
            "thread/started" => self.reduce_thread_started(notification.params, occurred_at),
            "thread/status/changed" => {
                self.reduce_thread_status_changed(notification.params, occurred_at)
            }
            "turn/started" => self.reduce_turn_started(notification.params, occurred_at),
            "turn/completed" => self.reduce_turn_completed(notification.params, occurred_at),
            "item/started" | "item/completed" => {
                self.reduce_item(notification.method, notification.params, occurred_at)
            }
            "item/agentMessage/delta" => self.reduce_delta(notification.params, occurred_at),
            "thread/tokenUsage/updated" => {
                self.reduce_token_usage(notification.params, occurred_at)
            }
            "serverRequest/resolved" => {
                self.reduce_server_request_resolved(notification.params, occurred_at)
            }
            "error" => self.reduce_error(notification.params, occurred_at),
            method => {
                let operation = approval_operation(method)?;
                self.reduce_approval(
                    operation,
                    notification.request_id,
                    notification.params,
                    occurred_at,
                )
            }
        }
    }

    fn reduce_thread_started(
        &mut self,
        params: Value,
        occurred_at: i64,
    ) -> Option<ReducedNotification> {
        let thread = params.get("thread")?;
        let thread_id = value_string(thread, "id")?;
        let status = brain_thread_status(thread.get("status"));
        let title = safe_optional_title(
            thread
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| thread.get("preview").and_then(Value::as_str)),
        );
        let mut reduced = self.event(
            "brain.threadStarted",
            Some(thread_id.clone()),
            None,
            None,
            None,
            Some(json!({ "status": brain_thread_status_name(&status) })),
            occurred_at,
        );
        reduced.thread_change = Some(ThreadChange {
            thread_id,
            title,
            model: None,
            status,
            occurred_at,
        });
        Some(reduced)
    }

    fn reduce_thread_status_changed(
        &mut self,
        params: Value,
        occurred_at: i64,
    ) -> Option<ReducedNotification> {
        let thread_id = value_string(&params, "threadId")?;
        let status = brain_thread_status(params.get("status"));
        let mut reduced = self.event(
            "brain.threadStatusChanged",
            Some(thread_id.clone()),
            None,
            None,
            None,
            Some(json!({ "status": brain_thread_status_name(&status) })),
            occurred_at,
        );
        reduced.thread_change = Some(ThreadChange {
            thread_id,
            title: None,
            model: None,
            status,
            occurred_at,
        });
        Some(reduced)
    }

    fn reduce_turn_started(
        &mut self,
        params: Value,
        occurred_at: i64,
    ) -> Option<ReducedNotification> {
        let thread_id = value_string(&params, "threadId")?;
        let remote_turn_id = params
            .get("turn")
            .and_then(|turn| value_string(turn, "id"))?;
        let local_turn_id = self.local_for_remote_or_pending(&thread_id, &remote_turn_id);
        let mut reduced = self.event(
            "brain.turnStarted",
            Some(thread_id.clone()),
            Some(local_turn_id),
            None,
            None,
            None,
            occurred_at,
        );
        reduced.thread_change = Some(ThreadChange {
            thread_id,
            title: None,
            model: None,
            status: BrainThreadStatus::Running,
            occurred_at,
        });
        Some(reduced)
    }

    fn reduce_delta(&mut self, params: Value, occurred_at: i64) -> Option<ReducedNotification> {
        let thread_id = value_string(&params, "threadId")?;
        let remote_turn_id = value_string(&params, "turnId")?;
        let item_id = value_string(&params, "itemId");
        let delta = params.get("delta")?.as_str().map(redact_stream_text)?;
        let local_turn_id = self.local_for_remote_or_pending(&thread_id, &remote_turn_id);
        if self.terminal_local_turns.contains(&local_turn_id) {
            return None;
        }
        if let Some(turn) = self.turns_by_remote.get_mut(&remote_turn_id) {
            append_bounded(&mut turn.assistant_text, &delta, MAX_ASSISTANT_CHARS);
        }
        Some(self.event(
            "brain.agentMessageDelta",
            Some(thread_id),
            Some(local_turn_id),
            item_id,
            Some(delta),
            None,
            occurred_at,
        ))
    }

    fn reduce_turn_completed(
        &mut self,
        params: Value,
        occurred_at: i64,
    ) -> Option<ReducedNotification> {
        let thread_id = value_string(&params, "threadId")?;
        let turn = params.get("turn")?;
        let remote_turn_id = value_string(turn, "id")?;
        let local_turn_id = self.local_for_remote_or_pending(&thread_id, &remote_turn_id);
        if !self.terminal_local_turns.insert(local_turn_id.clone()) {
            return None;
        }
        let status = brain_turn_status(turn.get("status"));
        let assistant_text = self
            .turns_by_remote
            .get(&remote_turn_id)
            .map(|turn| turn.assistant_text.clone())
            .unwrap_or_default();
        let error = (status == BrainTurnStatus::Failed).then(|| {
            turn.get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(safe_error_message)
                .filter(|message| !message.is_empty())
                .or_else(|| {
                    self.turns_by_remote
                        .get(&remote_turn_id)
                        .and_then(|turn| turn.error.clone())
                })
                .unwrap_or_else(|| "Codex turn failed".to_string())
        });
        let mut reduced = self.event(
            "brain.turnCompleted",
            Some(thread_id.clone()),
            Some(local_turn_id.clone()),
            None,
            None,
            Some(json!({ "status": brain_turn_status_name(&status) })),
            occurred_at,
        );
        reduced.thread_change = Some(ThreadChange {
            thread_id,
            title: None,
            model: None,
            status: if status == BrainTurnStatus::Failed {
                BrainThreadStatus::Error
            } else {
                BrainThreadStatus::Ready
            },
            occurred_at,
        });
        reduced.completion = Some(TurnCompletion {
            local_turn_id,
            status,
            assistant_text,
            error,
            occurred_at,
        });
        Some(reduced)
    }

    fn reduce_error(&mut self, params: Value, occurred_at: i64) -> Option<ReducedNotification> {
        let thread_id = value_string(&params, "threadId");
        let remote_turn_id = value_string(&params, "turnId");
        let message = params
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .or_else(|| params.get("message").and_then(Value::as_str))
            .map(safe_error_message)
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| "Codex runtime reported an error".to_string());
        let will_retry = params
            .get("willRetry")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let local_turn_id = remote_turn_id.as_ref().and_then(|remote_turn_id| {
            self.local_for_remote_or_pending_opt(thread_id.as_deref(), remote_turn_id)
        });
        if !will_retry {
            if let Some(remote_turn_id) = remote_turn_id.as_ref() {
                if let Some(turn) = self.turns_by_remote.get_mut(remote_turn_id) {
                    turn.error = Some(message.clone());
                }
            }
        }
        Some(self.event(
            "brain.error",
            thread_id,
            local_turn_id,
            None,
            None,
            Some(json!({ "message": message, "willRetry": will_retry })),
            occurred_at,
        ))
    }

    fn reduce_item(
        &mut self,
        method: String,
        params: Value,
        occurred_at: i64,
    ) -> Option<ReducedNotification> {
        let thread_id = value_string(&params, "threadId")?;
        let remote_turn_id = value_string(&params, "turnId")?;
        let item = params.get("item")?;
        let item_id = value_string(item, "id");
        let item_type = item
            .get("type")
            .and_then(Value::as_str)
            .map(safe_label)
            .unwrap_or_else(|| "unknown".to_string());
        let local_turn_id = self.local_for_remote_or_pending(&thread_id, &remote_turn_id);
        Some(self.event(
            if method == "item/started" {
                "brain.itemStarted"
            } else {
                "brain.itemCompleted"
            },
            Some(thread_id),
            Some(local_turn_id),
            item_id,
            None,
            Some(json!({ "itemType": item_type })),
            occurred_at,
        ))
    }

    fn reduce_token_usage(
        &mut self,
        params: Value,
        occurred_at: i64,
    ) -> Option<ReducedNotification> {
        self.event(
            "brain.tokenUsageUpdated",
            value_string(&params, "threadId"),
            value_string(&params, "turnId"),
            None,
            None,
            None,
            occurred_at,
        )
        .into()
    }

    fn reduce_server_request_resolved(
        &mut self,
        params: Value,
        occurred_at: i64,
    ) -> Option<ReducedNotification> {
        let thread_id = value_string(&params, "threadId")?;
        Some(self.event(
            "brain.serverRequestResolved",
            Some(thread_id),
            None,
            None,
            None,
            None,
            occurred_at,
        ))
    }

    fn reduce_approval(
        &mut self,
        operation: &'static str,
        request_id: Option<Value>,
        params: Value,
        occurred_at: i64,
    ) -> Option<ReducedNotification> {
        let request_id = safe_request_id(request_id?)?;
        let thread_id = value_string(&params, "threadId");
        let remote_turn_id = value_string(&params, "turnId");
        let local_turn_id = remote_turn_id
            .as_ref()
            .and_then(|remote| self.local_for_remote_or_pending_opt(thread_id.as_deref(), remote));
        let mut reduced = self.event(
            "brain.approvalRequired",
            thread_id.clone(),
            local_turn_id,
            value_string(&params, "itemId"),
            None,
            Some(json!({ "requestType": operation, "allowed": false })),
            occurred_at,
        );
        reduced.approval = Some(ApprovalIntent {
            operation,
            resource_id: request_id,
            thread_id,
            remote_turn_id,
        });
        Some(reduced)
    }

    fn local_for_remote_or_pending(&mut self, thread_id: &str, remote_turn_id: &str) -> String {
        if let Some(turn) = self.turns_by_remote.get(remote_turn_id) {
            return turn.local_turn_id.clone();
        }
        let pending = self
            .pending_by_thread
            .get_mut(thread_id)
            .and_then(VecDeque::pop_front);
        if self
            .pending_by_thread
            .get(thread_id)
            .is_some_and(VecDeque::is_empty)
        {
            self.pending_by_thread.remove(thread_id);
        }
        let local_turn_id = pending.unwrap_or_else(|| remote_turn_id.to_string());
        self.remote_by_local
            .insert(local_turn_id.clone(), remote_turn_id.to_string());
        self.turns_by_remote.insert(
            remote_turn_id.to_string(),
            TurnAccumulator {
                local_turn_id: local_turn_id.clone(),
                assistant_text: String::new(),
                error: None,
            },
        );
        local_turn_id
    }

    fn local_for_remote_or_pending_opt(
        &mut self,
        thread_id: Option<&str>,
        remote_turn_id: &str,
    ) -> Option<String> {
        if let Some(turn) = self.turns_by_remote.get(remote_turn_id) {
            return Some(turn.local_turn_id.clone());
        }
        thread_id.map(|thread_id| self.local_for_remote_or_pending(thread_id, remote_turn_id))
    }

    #[allow(clippy::too_many_arguments)]
    fn event(
        &mut self,
        event_type: &str,
        thread_id: Option<String>,
        turn_id: Option<String>,
        item_id: Option<String>,
        delta: Option<String>,
        payload: Option<Value>,
        occurred_at: i64,
    ) -> ReducedNotification {
        self.sequence = self.sequence.saturating_add(1);
        ReducedNotification {
            event: BrainStreamEvent {
                sequence: self.sequence,
                event_type: event_type.to_string(),
                thread_id,
                turn_id,
                item_id,
                delta,
                payload,
                occurred_at,
            },
            thread_change: None,
            completion: None,
            approval: None,
        }
    }
}

fn thread_from_response(
    response: &Value,
    project_id: Option<String>,
    title: Option<String>,
    requested_model: Option<String>,
) -> Result<BrainThreadRecord, HostError> {
    let thread = response
        .get("thread")
        .ok_or_else(|| protocol_error("thread response did not contain a thread"))?;
    let model = requested_model.or_else(|| {
        response
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    thread_from_value(thread, project_id, title, model)
}

fn thread_from_value(
    thread: &Value,
    project_id: Option<String>,
    title: Option<String>,
    model: Option<String>,
) -> Result<BrainThreadRecord, HostError> {
    let id = value_string(thread, "id")
        .ok_or_else(|| protocol_error("Codex thread has no stable id"))?;
    validate_id("remoteThreadId", &id)?;
    let now = now_millis();
    let created_at = seconds_to_millis(thread.get("createdAt")).unwrap_or(now);
    let updated_at = seconds_to_millis(thread.get("updatedAt")).unwrap_or(created_at);
    let title = title.or_else(|| {
        safe_optional_title(
            thread
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| thread.get("preview").and_then(Value::as_str)),
        )
    });
    Ok(BrainThreadRecord {
        id,
        project_id,
        title,
        model,
        status: brain_thread_status(thread.get("status")),
        created_at,
        updated_at,
    })
}

fn brain_thread_status(value: Option<&Value>) -> BrainThreadStatus {
    match value
        .and_then(|value| value.get("type").or(Some(value)))
        .and_then(Value::as_str)
    {
        Some("active") => BrainThreadStatus::Running,
        Some("systemError") => BrainThreadStatus::Error,
        _ => BrainThreadStatus::Ready,
    }
}

fn brain_turn_status(value: Option<&Value>) -> BrainTurnStatus {
    match value.and_then(Value::as_str) {
        Some("completed") => BrainTurnStatus::Completed,
        Some("interrupted") => BrainTurnStatus::Interrupted,
        Some("failed") => BrainTurnStatus::Failed,
        _ => BrainTurnStatus::Failed,
    }
}

fn brain_thread_status_name(status: &BrainThreadStatus) -> &'static str {
    match status {
        BrainThreadStatus::Ready => "ready",
        BrainThreadStatus::Running => "running",
        BrainThreadStatus::Error => "error",
        BrainThreadStatus::Archived => "archived",
    }
}

fn brain_turn_status_name(status: &BrainTurnStatus) -> &'static str {
    match status {
        BrainTurnStatus::Running => "running",
        BrainTurnStatus::Completed => "completed",
        BrainTurnStatus::Interrupted => "interrupted",
        BrainTurnStatus::Failed => "failed",
    }
}

fn approval_operation(method: &str) -> Option<&'static str> {
    Some(match method {
        "item/commandExecution/requestApproval" => "brain.commandExecution",
        "item/fileChange/requestApproval" => "brain.fileChange",
        "item/permissions/requestApproval" => "brain.requestPermissions",
        "item/tool/requestUserInput" => "brain.requestUserInput",
        "mcpServer/elicitation/request" => "brain.mcpElicitation",
        "account/chatgptAuthTokens/refresh" => "brain.authTokenRefresh",
        "attestation/generate" => "brain.attestation",
        "applyPatchApproval" => "brain.legacyApplyPatch",
        "execCommandApproval" => "brain.legacyExecCommand",
        _ => return None,
    })
}

fn business_dynamic_tool_specs() -> Vec<DynamicToolSpec> {
    let tools = BusinessToolRegistry::definitions()
        .into_iter()
        .map(|definition| {
            DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
                name: definition.name,
                description: definition.description,
                input_schema: definition.input_schema,
                defer_loading: false,
            })
        })
        .collect();
    vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: BUSINESS_TOOL_NAMESPACE.to_string(),
        description: "半山商务工作台的项目、合同文档、Artifact 与审批工具".to_string(),
        tools,
    })]
}

fn fixed_developer_instructions() -> String {
    format!(
        "Operate only inside the BSAIGC-managed workspace. Never expose local paths, credentials, provider configuration, or raw tool arguments. Request approval before any irreversible action.\n\n{BUSINESS_SYSTEM_PROMPT}"
    )
}

fn health_from_runtime(
    health: Option<&CodexRuntimeHealth>,
    subscribers: usize,
    last_error_code: Option<String>,
) -> BrainHostHealth {
    let Some(health) = health else {
        return BrainHostHealth {
            state: if last_error_code.is_some() {
                "unavailable".to_string()
            } else {
                "stopped".to_string()
            },
            running: false,
            initialized: false,
            pending_requests: 0,
            subscribers,
            started_at: None,
            last_message_at: None,
            last_error_code,
        };
    };
    BrainHostHealth {
        state: if health.running && health.initialized {
            "ready".to_string()
        } else {
            "degraded".to_string()
        },
        running: health.running,
        initialized: health.initialized,
        pending_requests: health.pending_requests,
        subscribers,
        started_at: Some(i64::try_from(health.started_at).unwrap_or(i64::MAX)),
        last_message_at: health
            .last_message_at
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
        last_error_code,
    }
}

fn map_non_replayable_error(method: &str, error: CodexRuntimeError) -> HostError {
    if outcome_is_unknown(&error) {
        HostError::new(
            "BRAIN_REQUEST_OUTCOME_UNKNOWN",
            format!("{method} outcome is unknown; the host will not replay it"),
            false,
        )
    } else {
        map_runtime_error(method, error, false)
    }
}

fn map_runtime_error(method: &str, error: CodexRuntimeError, retryable: bool) -> HostError {
    let code = match error {
        CodexRuntimeError::Unavailable(_) | CodexRuntimeError::SpawnFailed => {
            "BRAIN_RUNTIME_UNAVAILABLE"
        }
        CodexRuntimeError::DeadlineExceeded { .. } => "BRAIN_RUNTIME_TIMEOUT",
        CodexRuntimeError::Remote { .. } => "BRAIN_RUNTIME_REJECTED",
        CodexRuntimeError::ProcessExited(_) => "BRAIN_RUNTIME_EXITED",
        CodexRuntimeError::ShuttingDown => "BRAIN_RUNTIME_STOPPING",
        CodexRuntimeError::NotInitialized => "BRAIN_RUNTIME_NOT_READY",
        CodexRuntimeError::StdioUnavailable(_)
        | CodexRuntimeError::Transport(_)
        | CodexRuntimeError::Protocol(_) => "BRAIN_RUNTIME_PROTOCOL",
    };
    HostError::new(code, format!("Codex {method} failed"), retryable)
}

fn outcome_is_unknown(error: &CodexRuntimeError) -> bool {
    matches!(
        error,
        CodexRuntimeError::DeadlineExceeded { .. }
            | CodexRuntimeError::ProcessExited(_)
            | CodexRuntimeError::Transport(_)
            | CodexRuntimeError::Protocol(_)
    )
}

fn protocol_error(message: &str) -> HostError {
    HostError::new("BRAIN_RUNTIME_PROTOCOL", message, false)
}

fn validate_id(name: &str, value: &str) -> Result<(), HostError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 256 || value.chars().any(char::is_control) {
        return Err(HostError::validation(format!(
            "{name} must be a non-empty stable identifier"
        )));
    }
    Ok(())
}

fn validate_optional_id(name: &str, value: Option<&str>) -> Result<(), HostError> {
    if let Some(value) = value {
        validate_id(name, value)?;
    }
    Ok(())
}

fn validate_optional_text(
    name: &str,
    value: Option<&str>,
    max_chars: usize,
) -> Result<(), HostError> {
    if let Some(value) = value {
        if value.chars().count() > max_chars || value.contains('\0') {
            return Err(HostError::validation(format!("{name} is too large")));
        }
    }
    Ok(())
}

fn validate_input(value: &str) -> Result<(), HostError> {
    if value.trim().is_empty() {
        return Err(HostError::validation("inputText is required"));
    }
    if value.chars().count() > MAX_INPUT_CHARS || value.contains('\0') {
        return Err(HostError::validation("inputText is too large"));
    }
    Ok(())
}

fn validate_model(model: Option<&str>) -> Result<(), HostError> {
    let Some(model) = model else {
        return Ok(());
    };
    if model.trim().is_empty()
        || model.chars().count() > MAX_MODEL_CHARS
        || !model
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-_:/".contains(character))
    {
        return Err(HostError::validation(
            "model contains unsupported characters",
        ));
    }
    Ok(())
}

fn validate_effort(effort: Option<&str>) -> Result<(), HostError> {
    if let Some(effort) = effort {
        if !matches!(effort, "minimal" | "low" | "medium" | "high" | "xhigh") {
            return Err(HostError::validation("effort is not supported"));
        }
    }
    Ok(())
}

fn safe_optional_title(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(MAX_TITLE_CHARS).collect())
}

fn safe_label(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || ".-_".contains(*character))
        .take(64)
        .collect()
}

fn safe_request_id(value: Value) -> Option<String> {
    match value {
        Value::String(value) => {
            validate_id("requestId", &value).ok()?;
            Some(value)
        }
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_string(value: &Value, key: &str) -> Option<String> {
    let value = value.get(key)?.as_str()?;
    validate_id(key, value).ok()?;
    Some(value.to_string())
}

fn seconds_to_millis(value: Option<&Value>) -> Option<i64> {
    let seconds = value?.as_i64()?;
    seconds.checked_mul(1_000)
}

fn append_bounded(target: &mut String, value: &str, max_chars: usize) {
    let remaining = max_chars.saturating_sub(target.chars().count());
    if remaining > 0 {
        target.extend(value.chars().take(remaining));
    }
}

fn safe_error_message(value: &str) -> String {
    redact_stream_text(value.trim())
        .chars()
        .take(MAX_ERROR_CHARS)
        .collect()
}

fn redact_stream_text(value: &str) -> String {
    let mut redact_next = false;
    value
        .split_inclusive(char::is_whitespace)
        .map(|segment| {
            let core = segment.trim_end_matches(char::is_whitespace);
            let whitespace = &segment[core.len()..];
            let trimmed = core.trim_matches(['\'', '"', '`', ',', ';', '(', ')', '[', ']']);
            let lower = trimmed.to_ascii_lowercase();
            let is_url = lower.starts_with("http://") || lower.starts_with("https://");
            let sensitive_assignment = !is_url
                && lower.split_once('=').is_some_and(|(key, _)| {
                    key.contains("token")
                        || key.contains("secret")
                        || key.contains("password")
                        || key.contains("credential")
                });
            let should_redact = redact_next
                || looks_like_absolute_path(trimmed)
                || (lower.starts_with("sk-") && trimmed.len() > 10)
                || (trimmed.starts_with("AKIA") && trimmed.len() >= 16)
                || sensitive_assignment;
            redact_next = lower == "bearer";
            let rendered = if should_redact {
                "[redacted]".to_string()
            } else if is_url && trimmed.contains('?') {
                trimmed.split('?').next().unwrap_or(trimmed).to_string()
            } else {
                core.to_string()
            };
            format!("{rendered}{whitespace}")
        })
        .collect()
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

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn lock_connection(
    connection: &Mutex<Connection>,
) -> Result<std::sync::MutexGuard<'_, Connection>, HostError> {
    connection
        .lock()
        .map_err(|_| HostError::internal("brain SQLite lock is poisoned"))
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(method: &str, params: Value) -> CodexNotification {
        CodexNotification {
            request_id: None,
            method: method.to_string(),
            params,
            received_at: 10,
        }
    }

    #[test]
    fn reducer_concatenates_delta_and_finishes_once() {
        let mut reducer = NotificationReducer::default();
        reducer.register_pending("thread-1", "local-1");
        reducer
            .reduce(notification(
                "turn/started",
                json!({
                    "threadId": "thread-1",
                    "turn": { "id": "remote-1", "status": "inProgress" }
                }),
            ))
            .unwrap();

        let first = reducer
            .reduce(notification(
                "item/agentMessage/delta",
                json!({
                    "threadId": "thread-1",
                    "turnId": "remote-1",
                    "itemId": "item-1",
                    "delta": "Hello "
                }),
            ))
            .unwrap();
        let second = reducer
            .reduce(notification(
                "item/agentMessage/delta",
                json!({
                    "threadId": "thread-1",
                    "turnId": "remote-1",
                    "itemId": "item-1",
                    "delta": "world"
                }),
            ))
            .unwrap();
        assert_eq!(first.event.delta.as_deref(), Some("Hello "));
        assert_eq!(second.event.delta.as_deref(), Some("world"));

        let completed = reducer
            .reduce(notification(
                "turn/completed",
                json!({
                    "threadId": "thread-1",
                    "turn": { "id": "remote-1", "status": "completed", "items": [] }
                }),
            ))
            .unwrap();
        let completion = completed.completion.unwrap();
        assert_eq!(completion.local_turn_id, "local-1");
        assert_eq!(completion.assistant_text, "Hello world");
        assert_eq!(completion.status, BrainTurnStatus::Completed);

        assert!(reducer
            .reduce(notification(
                "turn/completed",
                json!({
                    "threadId": "thread-1",
                    "turn": { "id": "remote-1", "status": "completed", "items": [] }
                }),
            ))
            .is_none());
    }

    #[test]
    fn reducer_persists_non_retryable_codex_error_message() {
        let mut reducer = NotificationReducer::default();
        reducer.register_pending("thread-1", "local-1");
        reducer
            .reduce(notification(
                "turn/started",
                json!({
                    "threadId": "thread-1",
                    "turn": { "id": "remote-1", "status": "inProgress" }
                }),
            ))
            .unwrap();

        let error = reducer
            .reduce(notification(
                "error",
                json!({
                    "threadId": "thread-1",
                    "turnId": "remote-1",
                    "willRetry": false,
                    "error": {
                        "message": "Missing environment variable: BSAIGC_CODEX_API_KEY"
                    }
                }),
            ))
            .unwrap();
        assert_eq!(
            error.event.payload.as_ref().unwrap()["message"],
            "Missing environment variable: BSAIGC_CODEX_API_KEY"
        );

        let completed = reducer
            .reduce(notification(
                "turn/completed",
                json!({
                    "threadId": "thread-1",
                    "turn": { "id": "remote-1", "status": "failed", "items": [] }
                }),
            ))
            .unwrap();
        assert_eq!(
            completed.completion.unwrap().error.as_deref(),
            Some("Missing environment variable: BSAIGC_CODEX_API_KEY")
        );
    }

    #[test]
    fn reducer_ignores_retryable_error_after_successful_retry() {
        let mut reducer = NotificationReducer::default();
        reducer.register_pending("thread-1", "local-1");
        reducer
            .reduce(notification(
                "turn/started",
                json!({
                    "threadId": "thread-1",
                    "turn": { "id": "remote-1", "status": "inProgress" }
                }),
            ))
            .unwrap();
        reducer
            .reduce(notification(
                "error",
                json!({
                    "threadId": "thread-1",
                    "turnId": "remote-1",
                    "willRetry": true,
                    "error": { "message": "temporary provider failure" }
                }),
            ))
            .unwrap();

        let completed = reducer
            .reduce(notification(
                "turn/completed",
                json!({
                    "threadId": "thread-1",
                    "turn": { "id": "remote-1", "status": "completed", "items": [] }
                }),
            ))
            .unwrap();
        assert_eq!(completed.completion.unwrap().error, None);
    }

    #[test]
    fn reducer_never_forwards_sensitive_notification_fields() {
        let mut reducer = NotificationReducer::default();
        let event = reducer
            .reduce(notification(
                "thread/started",
                json!({
                    "thread": {
                        "id": "thread-1",
                        "status": { "type": "idle" },
                        "cwd": "C:\\Users\\operator\\secret",
                        "path": "C:\\Users\\operator\\thread.jsonl",
                        "codexHome": "C:\\Users\\operator\\.codex",
                        "token": "sk-super-secret-value",
                        "config": { "provider": "private" }
                    }
                }),
            ))
            .unwrap()
            .event;
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(!serialized.contains("Users"));
        assert!(!serialized.contains("sk-super"));
        assert!(!serialized.contains("provider"));
        assert!(!serialized.contains("codexHome"));
    }

    #[test]
    fn approval_request_is_reduced_without_raw_arguments() {
        let mut reducer = NotificationReducer::default();
        reducer.register_pending("thread-1", "local-1");
        let mut request = notification(
            "item/commandExecution/requestApproval",
            json!({
                "threadId": "thread-1",
                "turnId": "remote-1",
                "itemId": "item-1",
                "command": "powershell -Command Get-Content C:\\secret.txt",
                "cwd": "C:\\Users\\operator"
            }),
        );
        request.request_id = Some(json!(77));
        let reduced = reducer.reduce(request).unwrap();
        assert_eq!(
            reduced.approval.as_ref().map(|value| value.operation),
            Some("brain.commandExecution")
        );
        let serialized = serde_json::to_string(&reduced.event).unwrap();
        assert!(!serialized.contains("powershell"));
        assert!(!serialized.contains("secret.txt"));
        assert!(!serialized.contains("operator"));
    }

    #[test]
    fn unknown_notification_is_ignored() {
        let mut reducer = NotificationReducer::default();
        assert!(reducer
            .reduce(notification(
                "future/privateNotification",
                json!({ "cwd": "C:\\secret", "token": "sk-secret" }),
            ))
            .is_none());
        assert_eq!(reducer.sequence, 0);
    }

    #[test]
    fn stream_text_redacts_paths_tokens_and_url_queries() {
        let redacted = redact_stream_text(
            "open C:\\Users\\operator\\x.txt with Bearer abc123 and https://host/x?token=y sk-1234567890abcdef",
        );
        assert!(!redacted.contains("operator"));
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("token=y"));
        assert!(!redacted.contains("sk-123"));
        assert!(redacted.contains("https://host/x"));
    }

    #[test]
    fn reopening_host_interrupts_running_turn_without_replay() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("brain.sqlite3");
        let workspace = directory.path().join("workspace");
        {
            let connection = Connection::open(&database).unwrap();
            brain_store::migrate(&connection).unwrap();
            brain_store::upsert_thread(
                &connection,
                &BrainThreadRecord {
                    id: "thread-recovery".to_string(),
                    project_id: None,
                    title: None,
                    model: None,
                    status: BrainThreadStatus::Running,
                    created_at: 1,
                    updated_at: 1,
                },
            )
            .unwrap();
            brain_store::insert_turn(
                &connection,
                &BrainTurnRecord {
                    id: "turn-recovery".to_string(),
                    thread_id: "thread-recovery".to_string(),
                    status: BrainTurnStatus::Running,
                    input_text: "do not replay me".to_string(),
                    assistant_text: String::new(),
                    error: None,
                    created_at: 1,
                    updated_at: 1,
                },
            )
            .unwrap();
        }

        let host = BrainHost::open(&database, &workspace).unwrap();
        let turn = host.list_local_turns("thread-recovery").unwrap().remove(0);
        assert_eq!(turn.status, BrainTurnStatus::Interrupted);
        assert!(turn.error.as_deref().unwrap().contains("not replayed"));
        let thread = host.list_local_threads(None).unwrap().remove(0);
        assert_eq!(thread.status, BrainThreadStatus::Error);
        assert!(!host.health().running);
    }

    #[test]
    fn business_dynamic_tools_publish_the_registry_allowlist() {
        let value = serde_json::to_value(business_dynamic_tool_specs()).unwrap();
        let namespace = &value[0];
        assert_eq!(namespace["type"], "namespace");
        assert_eq!(namespace["name"], BUSINESS_TOOL_NAMESPACE);
        let tools = namespace["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 14);
        assert_eq!(tools[0]["name"], "project_read");
        assert!(tools.iter().any(|tool| tool["name"] == "document_validate"));
        assert!(tools.iter().all(|tool| tool["type"] == "function"));
    }

    #[test]
    fn dynamic_tool_calls_are_not_misclassified_as_approvals() {
        assert_eq!(approval_operation("item/tool/call"), None);
        assert_eq!(
            approval_operation("item/fileChange/requestApproval"),
            Some("brain.fileChange")
        );
    }

    #[test]
    fn dynamic_tool_dispatch_returns_structured_result_without_raw_protocol_errors() {
        struct Adapter;
        impl crate::business_tool_registry::BusinessToolDispatchAdapter for Adapter {
            fn project_read(
                &self,
                _context: &crate::business_tool_registry::BusinessToolContext,
                input: crate::business_tool_registry::ProjectReadInput,
            ) -> Result<
                crate::business_tool_registry::ProjectReadOutput,
                crate::business_tool_registry::BusinessToolError,
            > {
                Ok(crate::business_tool_registry::ProjectReadOutput {
                    project: crate::business_tool_registry::BusinessProjectView {
                        id: input.project_id,
                        name: "测试项目".to_string(),
                        client_name: "测试客户".to_string(),
                        stage: "briefing".to_string(),
                        revision: 1,
                        updated_at: 1,
                        brief: Default::default(),
                    },
                    business_workspace: None,
                })
            }

            fn artifact_read(
                &self,
                _context: &crate::business_tool_registry::BusinessToolContext,
                _input: crate::business_tool_registry::ArtifactReadInput,
            ) -> Result<
                crate::business_tool_registry::ArtifactReadOutput,
                crate::business_tool_registry::BusinessToolError,
            > {
                Err(
                    crate::business_tool_registry::BusinessToolError::adapter_unavailable(
                        "artifact_read",
                    ),
                )
            }

            fn document_extract(
                &self,
                _context: &crate::business_tool_registry::BusinessToolContext,
                _input: crate::business_tool_registry::DocumentExtractInput,
            ) -> Result<
                crate::business_tool_registry::DocumentExtractOutput,
                crate::business_tool_registry::BusinessToolError,
            > {
                Err(
                    crate::business_tool_registry::BusinessToolError::adapter_unavailable(
                        "document_extract",
                    ),
                )
            }

            fn artifact_create(
                &self,
                _context: &crate::business_tool_registry::BusinessToolContext,
                _input: crate::business_tool_registry::ArtifactCreateInput,
            ) -> Result<
                crate::business_tool_registry::ArtifactCreateOutput,
                crate::business_tool_registry::BusinessToolError,
            > {
                Err(
                    crate::business_tool_registry::BusinessToolError::adapter_unavailable(
                        "artifact_create",
                    ),
                )
            }

            fn approval_request(
                &self,
                _context: &crate::business_tool_registry::BusinessToolContext,
                _input: crate::business_tool_registry::ApprovalRequestInput,
            ) -> Result<
                crate::business_tool_registry::ApprovalRequestOutput,
                crate::business_tool_registry::BusinessToolError,
            > {
                Err(
                    crate::business_tool_registry::BusinessToolError::adapter_unavailable(
                        "approval_request",
                    ),
                )
            }
        }

        let response = dispatch_dynamic_tool(
            &BusinessToolRegistry::new(Adapter),
            Some("project-1".to_string()),
            DynamicToolCallRequest {
                request_id: CodexRequestId::Integer(80),
                params: crate::codex_runtime::DynamicToolCallParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    call_id: "call-1".to_string(),
                    namespace: Some(BUSINESS_TOOL_NAMESPACE.to_string()),
                    tool: "project_read".to_string(),
                    arguments: json!({ "projectId": "project-1" }),
                },
            },
        );
        assert!(response.success);
        let DynamicToolCallOutputContentItem::InputText { text } = &response.content_items[0]
        else {
            panic!("dynamic business tool response must be text JSON");
        };
        let value: Value = serde_json::from_str(text).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["result"]["tool"], "project_read");
    }
}
